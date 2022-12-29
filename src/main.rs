use clap::Parser;
use log::{debug, error, trace};
use std::process::Stdio;
use std::{env, ffi::OsStr, iter::once};
use thirtyfour::{common::capabilities::firefox::FirefoxPreferences, prelude::*};
use tokio::{
    process::Command,
    signal::{self},
    sync::broadcast::{channel, Receiver, Sender},
};

#[cfg(unix)]
const GECKODRIVER_EXEC: &str = "geckodriver";

#[cfg(unix)]
const PATH_VAR_SEPARATOR: char = ':';

#[cfg(windows)]
const GECKODRIVER_EXEC: &str = "geckodriver.exe";

#[cfg(windows)]
const PATH_VAR_SEPARATOR: char = ';';

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
    show_windows: bool,

    #[arg(short, long, default_value_t = 4444, help = "Sets the port that is used to communicate with geckodriver", value_parser = clap::value_parser!(u16).range(1..))]
    port: u16,
}

#[tokio::main]
async fn main() -> WebDriverResult<()> {
    setup_logger().unwrap();

    let cli = Cli::parse();

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
        cli.show_windows,
        cli.port,
        proc_sync_tx.clone(),
        proc_sync_tx.subscribe(),
        task_sync_tx,
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
    driver_fut.await.unwrap().unwrap();
    gecko_fut.await.unwrap().unwrap();

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
    mut task_sync_rx: Receiver<Signal>,
) -> Result<(), String> {
    debug!("Attempting to start geckodriver");
    let mut fut = Command::new(GECKODRIVER_EXEC)
        .args(["-p", &port.to_string()])
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

    let driver = WebDriver::new(&format!("http://localhost:{port}"), caps)
        .await
        .unwrap();

    let task = tokio::spawn({
        let local_driver = driver.clone();
        async move {
            if login(local_driver).await.is_err() {
                return Err("Unable to perform login".to_string());
            }

            // Inform all the other tasks that the downloading has been finished
            #[allow(unused_must_use)]
            {
                // If the result is an error that just means that that the channel has been closed already
                proc_sync_tx.send(Signal {});
            }
            Ok(())
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

async fn login(driver: WebDriver) -> WebDriverResult<()> {
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
    // TODO: Fix this
    login_input.send_keys("###########").await?;

    debug!("Submitting first stage login");
    let login_button = login_form.query(By::Tag("button")).with_attribute("type", "submit").first().await?;
    login_button.wait_until().clickable().await?;
    login_button.click().await?;
    debug!("First stage login form submitted");

    debug!("Changing login method from BankID to PIN and OTP");
    let login_type = driver.query(By::Tag("div")).with_id("r_state-2").first().await?;
    login_type.wait_until().clickable().await?;
    login_type.click().await?;

    let pin_input = driver.query(By::Id("phoneCode")).first().await?;
    let pin = rpassword::read_password().unwrap();

    pin_input.send_keys(&pin).await?;

    Ok(())
}
