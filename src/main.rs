use chrono::{Datelike, Local, Month, NaiveDate};
use clap::Parser;
use config::{Account, Config};
use inquire::validator::Validation;
use inquire::{Password, PasswordDisplayMode, Text};
use log::{debug, error, info, trace, warn};
use num_traits::FromPrimitive;
use std::collections::HashMap;
use std::iter::repeat;
use std::path::PathBuf;
use std::process::Stdio;
use std::{env, ffi::OsStr, iter::once};
use thirtyfour::components::SelectElement;
use thirtyfour::{common::capabilities::firefox::FirefoxPreferences, prelude::*};
use tokio::{
    join,
    process::Command,
    signal::{self},
    sync::broadcast::{channel, Receiver, Sender},
};

mod config;
mod system;

#[cfg(unix)]
const GECKODRIVER_EXEC: &str = "geckodriver";

#[cfg(unix)]
const PATH_VAR_SEPARATOR: char = ':';

#[cfg(windows)]
const GECKODRIVER_EXEC: &str = "geckodriver.exe";

#[cfg(windows)]
const PATH_VAR_SEPARATOR: char = ';';

#[derive(Debug)]
enum AccountStatementStatus {
    Downloaded,
    NotFound,
}

#[derive(Debug, Clone, PartialEq)]
struct Signal {}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(
        long,
        default_value_t = false,
        help = "Controls whether to display the various windows which may appear during the procedure."
    )]
    show: bool,

    #[arg(short, long, default_value_t = 4444, help = "Sets the port that is used to communicate with geckodriver", value_parser = clap::value_parser!(u16).range(1..))]
    port: u16,

    #[arg(help = "The path to the config file")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> WebDriverResult<()> {
    setup_logger().unwrap();

    let cli = Cli::parse();

    let config = config::read_config(&cli.config).unwrap();

    // Used to distribute a signal between the
    let (proc_sync_tx, mut proc_sync_rx) = channel::<Signal>(1);
    let (task_sync_tx, _) = channel::<Signal>(1);

    add_geckodriver_to_path().unwrap();

    // Start Geckodriver
    let gecko_fut = tokio::spawn(run_geckodriver(
        cli.port,
        proc_sync_tx.subscribe(),
        task_sync_tx.subscribe(),
    ));

    let driver_fut = tokio::spawn(run_driver(
        cli.show,
        cli.port,
        proc_sync_tx.clone(),
        proc_sync_tx.subscribe(),
        task_sync_tx,
        config,
    ));

    let signal_fut = tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {}
            Err(err) => {
                eprintln!("Unable to listen for shutdown signal: {err}");
            }
        }

        // Send a signal to shut down all the async procs
        proc_sync_tx.send(Signal {}).unwrap();
    });

    proc_sync_rx.recv().await.unwrap();

    // There is no point in awaiting this task as it doesn't hold any resources
    signal_fut.abort();

    // It is OK to ignore the results of these tasks even though they do return a result
    #[allow(unused_must_use)]
    {
        join!(driver_fut, gecko_fut);
    }

    Ok(())
}

fn setup_logger() -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .chain(std::io::stderr())
        .chain(fern::log_file("output.log")?)
        .apply()?;
    Ok(())
}

fn add_geckodriver_to_path() -> Result<(), String> {
    let mut dir = env::current_dir().unwrap();
    dir.push("drivers");

    if !dir.exists() {
        error!(
            "Unable to locate directory for geckodriver executable in: {}",
            dir.parent().unwrap().display()
        );
        return Err("Unable to locate geckodriver".to_string());
    }

    dir.push(env::consts::OS);
    if !dir.exists() {
        error!(
            "Unable to locate OS directory for geckodriver executable in: {}",
            dir.parent().unwrap().display()
        );
        return Err("Unable to locate geckodriver".to_string());
    }

    dir.push(GECKODRIVER_EXEC);
    if !dir.exists() {
        error!(
            "Unable to locate OS directory for geckodriver executable in: {}",
            dir.parent().unwrap().display()
        );
        return Err("Unable to locate geckodriver".to_string());
    }

    debug!("Located geckodriver executable at: {}", dir.display());
    trace!("Current PATH variable: {}", env::var("PATH").unwrap());

    env::set_var(
        "PATH",
        env::join_paths(
            once(dir.parent().unwrap().as_os_str()).chain(
                env::var("PATH")
                    .unwrap()
                    .split(PATH_VAR_SEPARATOR)
                    .into_iter()
                    .map(OsStr::new),
            ),
        )
        .unwrap(),
    );

    trace!("New PATH variable: {}", env::var("PATH").unwrap());

    Ok(())
}

async fn run_geckodriver(
    port: u16,
    mut proc_sync_rx: Receiver<Signal>,
    mut _task_sync_rx: Receiver<Signal>,
) -> Result<(), String> {
    debug!("Attempting to start geckodriver");
    let mut fut = Command::new(GECKODRIVER_EXEC)
        .args([
            "-p",
            &port.to_string(),
            "-b",
            system::find_firefox()
                .unwrap()
                .as_os_str()
                .to_str()
                .unwrap(),
        ])
        .kill_on_drop(true)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn");
    debug!("Geckodriver started, waiting for shutdown signal");

    debug!("Waiting for signal to shutdown geckodriver");
    proc_sync_rx.recv().await.unwrap();
    debug!("Received process wide shutdown signal");

    // debug!("Waiting for signal that the WebDriver has been shut down");
    // task_sync_rx.recv().await.unwrap();
    // debug!("Received WebDriver done signal");

    debug!("Shutdown signal received. Killing geckodriver");
    match fut.try_wait() {
        Ok(Some(_)) => {}
        Ok(None) => {
            fut.kill().await.unwrap();
        }
        Err(err) => {
            println!("Error occured when reaping geckodriver: {err}");
        }
    }

    Ok(())
}

async fn run_driver(
    show_windows: bool,
    port: u16,
    proc_sync_tx: Sender<Signal>,
    mut proc_sync_rx: Receiver<Signal>,
    task_sync_tx: Sender<Signal>,
    config: Config,
) -> Result<(), String> {
    let mut profile = FirefoxPreferences::default();
    profile.set("browser.download.folderList", 2).unwrap();
    profile
        .set("browser.download.manager.showWhenStarting", false)
        .unwrap();
    profile
        .set("browser.download.dir", env::current_dir().unwrap())
        .unwrap();
    profile
        .set("browser.helperApps.neverAsk.saveToDisk", "application/pdf")
        .unwrap();
    profile.set("pdfjs.disabled", true).unwrap();
    profile.set("plugin.scan.plid.all", false).unwrap();
    profile.set("plugin.scan.Acrobat", "99.0").unwrap();
    profile.set("general.warnOnAboutConfig", false).unwrap();

    let mut caps = DesiredCapabilities::firefox();
    caps.set_preferences(profile).unwrap();

    if !show_windows {
        caps.set_headless().unwrap();
    }

    let driver = match WebDriver::new(&format!("http://localhost:{port}"), caps).await {
        Ok(d) => d,
        Err(err) => {
            error!("Unable to start webdriver: {}", err);

            // Inform all the other tasks that to shut down
            #[allow(unused_must_use)]
            {
                task_sync_tx.send(Signal {});
                proc_sync_tx.send(Signal {});
            }
            return Err("Error occured when starting webdriver".to_string());
        }
    };

    let task = tokio::spawn({
        let local_driver = driver.clone();
        async move {
            initial(&local_driver)
                .await
                .expect("Unable to perform initial step for login");
            first_login_stage(&local_driver, &config)
                .await
                .expect("Unable to perform first login stage");
            second_login_stage(&local_driver)
                .await
                .expect("Unable to perform second login stage");

            // This is to avoid issues with prompts that appear right after you log in
            debug!("Attempting to navigate to user home");
            let logo = local_driver
                .query(By::Tag("a"))
                .with_attribute("title", "DNB")
                .first()
                .await
                .unwrap();
            logo.wait_until().clickable().await.unwrap();
            logo.click().await.unwrap();

            navigate_to_account_statements(&local_driver)
                .await
                .expect("Unable to navigate to account statements");

            download_statements(&local_driver, &config).await.unwrap();

            // Inform all the other tasks that the downloading has been finished
            #[allow(unused_must_use)]
            {
                // If the result is an error that just means that that the channel has been closed already
                proc_sync_tx.send(Signal {});
            }
        }
    });

    debug!("Waiting for signal to shutdown task");
    proc_sync_rx.recv().await.unwrap();

    if !task.is_finished() {
        task.abort();
    }

    driver.quit().await.unwrap();
    task_sync_tx.send(Signal {}).unwrap();
    Ok(())
}

async fn initial(driver: &WebDriver) -> WebDriverResult<()> {
    driver.goto("https://dnb.no").await?;

    debug!("Awaiting the consent modal");
    let consent_modal = driver.query(By::Id("consent-modal")).first().await?;
    consent_modal.wait_until().displayed().await?;
    debug!("Consent modal located");

    debug!("Attempting to locate close button for modal");
    let modal_close = consent_modal
        .query(By::Tag("button"))
        .with_class("consent-close")
        .first()
        .await?;
    modal_close.wait_until().clickable().await?;
    debug!("Close button for modal is now clickable");
    modal_close.click().await?;

    Ok(())
}

async fn first_login_stage(driver: &WebDriver, config: &Config) -> WebDriverResult<()> {
    debug!("Attempting to trigger login modal");
    let login_button = driver
        .query(By::Tag("span"))
        .with_text("Logg inn")
        .first()
        .await?;
    login_button.wait_until().clickable().await?;
    login_button.click().await?;

    debug!("Waiting for login modal to appear");
    let login_modal = driver.query(By::Id("dnb-modal-root")).first().await?;
    login_modal.wait_until().displayed().await?;
    debug!("Login modal is now displayed");

    debug!("Attempting to fill in login form");
    let login_form = login_modal.query(By::Tag("form")).first().await?;

    debug!("Entering SSN into form");
    let login_input = login_form
        .query(By::Tag("input"))
        .with_attribute("name", "uid")
        .first()
        .await?;

    let ssn = match config.ssn.clone() {
        Some(s) => s,
        None => Text::new("SSN (11 digits):").prompt().unwrap(),
    };
    login_input.send_keys(&ssn).await?;

    debug!("Submitting first stage login");
    let login_button = login_form
        .query(By::Tag("button"))
        .with_attribute("type", "submit")
        .first()
        .await?;
    login_button.wait_until().clickable().await?;
    login_button.click().await?;
    debug!("First stage login form submitted");

    Ok(())
}

async fn second_login_stage(driver: &WebDriver) -> WebDriverResult<()> {
    debug!("Changing login method from BankID to PIN and OTP");
    let parent_container = driver
        .query(By::Tag("div"))
        .with_id("r_state-2")
        .first()
        .await?;
    let login_type = parent_container
        .query(By::Tag("div"))
        .with_attribute("role", "button")
        .first()
        .await?;
    login_type.wait_until().clickable().await?;
    login_type.click().await?;
    debug!("Switched to PIN and OTP");

    debug!("Locating login form elements");
    let login_form = parent_container.query(By::Tag("form")).first().await?;

    let pin_input = login_form.query(By::Id("phoneCode")).first().await?;
    let otp_input = login_form.query(By::Id("otpCode")).first().await?;
    let login_button = login_form
        .query(By::Tag("button"))
        .with_attribute("type", "submit")
        .first()
        .await?;

    debug!("Asking user for PIN and OTP");
    let pin = Password::new("PIN (4 digits):")
        .without_confirmation()
        .with_display_mode(PasswordDisplayMode::Masked)
        .with_formatter(&|s| "*".repeat(s.len()))
        .with_validator(|s: &str| {
            if s.len() != 4 {
                return Ok(Validation::Invalid(
                    "PIN needs to be exactly 4 characters long".into(),
                ));
            }

            if !s.chars().all(char::is_numeric) {
                return Ok(Validation::Invalid(
                    "PIN can only contain numerical digits".into(),
                ));
            }

            Ok(Validation::Valid)
        })
        .prompt()
        .unwrap();

    let otp = Password::new("One time password (6 digits):")
        .without_confirmation()
        .with_display_mode(PasswordDisplayMode::Full)
        .with_formatter(&|s| s.to_string())
        .with_validator(|s: &str| {
            if s.len() != 6 {
                return Ok(Validation::Invalid(
                    "OTP needs to be exactly 6 characters long".into(),
                ));
            }

            if !s.chars().all(char::is_numeric) {
                return Ok(Validation::Invalid(
                    "OTP can only contain numerical digits".into(),
                ));
            }

            Ok(Validation::Valid)
        })
        .prompt()
        .unwrap();
    debug!("User PIN and OTP validated successfully");

    pin_input.send_keys(&pin).await?;
    otp_input.send_keys(&otp).await?;

    debug!("Submitting user login");
    login_button.wait_until().clickable().await?;
    login_button.click().await?;

    Ok(())
}

async fn navigate_to_account_statements(driver: &WebDriver) -> WebDriverResult<()> {
    debug!("Attempting to navigate to account statements");
    let site_menu = driver
        .query(By::Tag("a"))
        .with_attribute("role", "button")
        .with_text("Dagligbank og lån")
        .first()
        .await?;
    let archive = driver
        .query(By::Tag("a"))
        .with_attribute("title", "Arkiv")
        .first()
        .await?;

    debug!("Waiting for site menu to be clickable");
    site_menu.wait_until().clickable().await?;
    site_menu.click().await?;

    debug!("Waiting for link to be clickable");
    archive.wait_until().clickable().await?;
    archive.click().await?;

    debug!("Waiting for archive site to be loaded");
    driver
        .query(By::Id("documentType-button"))
        .first()
        .await?
        .wait_until()
        .clickable()
        .await?;

    debug!("Executing custom javascript to display document selector");
    driver
        .execute(
            r#"document.getElementById("documentType").style = "display: block;""#,
            vec![],
        )
        .await?;

    debug!("Waiting for archive menu to be accessible");
    let archive_menu = driver
        .query(By::Tag("select"))
        .with_id("documentType")
        .first()
        .await?;
    archive_menu.wait_until().displayed().await?;

    let archive_menu = SelectElement::new(&archive_menu).await?;
    archive_menu.select_by_value("kontoutskrift").await?;

    Ok(())
}

async fn download_statements<'a>(
    driver: &WebDriver,
    config: &'a Config,
) -> WebDriverResult<HashMap<&'a String, Vec<AccountStatementStatus>>> {
    let mut tmp_results: Vec<(&String, Vec<AccountStatementStatus>)> = Vec::new();
    for (account, (from, to)) in config
        .extractions
        .iter()
        .flat_map(|e| e.accounts.iter().zip(repeat((e.from, e.to))))
    {
        let download_results = download_account_statements(&driver, account, from, to)
            .await
            .unwrap();
        tmp_results.push((&account.id, download_results));
    }

    Ok(HashMap::from_iter(tmp_results.into_iter()))
}

fn month_number(date: NaiveDate) -> u32 {
    let today = Local::now().date_naive();

    today.years_since(date).unwrap() * 12 + (today.month() - date.month())
}

async fn download_account_statements(
    driver: &WebDriver,
    account: &Account,
    start: NaiveDate,
    stop: NaiveDate,
) -> WebDriverResult<Vec<AccountStatementStatus>> {
    let month_indices = month_number(start)..month_number(stop);
    let mut downloads: Vec<AccountStatementStatus> = Vec::with_capacity(month_indices.len());

    debug!("Attempting to download statements for {}", account.id);
    debug!("Waiting for account selector to be displayed");
    driver
        .query(By::Id("accountNumber-button"))
        .first()
        .await?
        .wait_until()
        .clickable()
        .await?;

    debug!("Executing custom javascript to display account selector");
    driver
        .execute(
            r#"document.getElementById("accountNumber").style = "display: block;""#,
            vec![],
        )
        .await?;

    debug!("Waiting for account selector to be accessible");
    let account_menu = driver
        .query(By::Tag("select"))
        .with_id("accountNumber")
        .first()
        .await?;
    account_menu.wait_until().displayed().await?;

    debug!("Attempting to select account {}", account.id);
    let account_menu = SelectElement::new(&account_menu).await?;
    account_menu
        .select_by_value(&account.id.replace('.', ""))
        .await?;

    debug!("Waiting for month selector to be displayed");
    driver
        .query(By::Id("searchIntervalIndex-button"))
        .first()
        .await?
        .wait_until()
        .clickable()
        .await?;

    debug!("Executing custom javascript to display month selector");
    driver
        .execute(
            r#"document.getElementById("searchIntervalIndex").style = "display: block;""#,
            vec![],
        )
        .await?;

    debug!("Waiting for month selector to be accessible");
    let month_menu = driver
        .query(By::Tag("select"))
        .with_id("searchIntervalIndex")
        .first()
        .await?;
    month_menu.wait_until().displayed().await?;
    let month_menu = SelectElement::new(&month_menu).await?;

    let retrieve_button = driver.query(By::Id("archiveSearchSubmit")).first().await?;

    let current_month = Month::from_u32(start.month()).unwrap();
    for (vec_index, month_index) in month_indices.enumerate() {
        debug!(
            "Attempting to download {} statements for {}",
            current_month.name(),
            account.id
        );
        month_menu.select_by_value(&month_index.to_string()).await?;

        debug!("Fetching statements for {}", current_month.name());
        retrieve_button.click().await?;

        debug!("Looking for query result");
        let result_elem = driver
            .query(By::Tag("h3"))
            .with_text("Søket ga ingen treff!")
            .or(By::LinkText("ajax/attachment/0/kontoutskrift"))
            .first()
            .await?;

        match result_elem.tag_name().await?.as_str() {
            "h3" => {
                warn!(
                    "The query looking for {} {} statements failed",
                    account.id,
                    current_month.name()
                );
            }
            "a" => {
                info!(
                    "The query looking for {} {} statements was successful",
                    account.id,
                    current_month.name()
                );
                result_elem.click().await?;
            }
            _ => unreachable!("Invalid tag name from result"),
        }

        // Move to the next month
        current_month.succ();
    }

    Ok(downloads)
}
