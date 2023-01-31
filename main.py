import os
import pathlib
import re
import sys
import traceback
from datetime import datetime, date
from inspect import getsourcefile
from dataclasses import dataclass
from typing import Optional

from dacite import from_dict, Config as DaConfig

import yaml
from PyPDF2 import PdfFileMerger
from selenium import webdriver
from selenium.common.exceptions import NoSuchElementException, TimeoutException
from selenium.webdriver.common.by import By
from selenium.webdriver.support import expected_conditions as EC
from selenium.webdriver.support.ui import Select, WebDriverWait

try:
    from pwinput import pwinput as getpass
except ImportError:
    from getpass import getpass

try:
    from yaml import CLoader as Loader
except ImportError:
    from yaml import Loader

@dataclass
class Account:
    id: str
    name: Optional[str]

@dataclass
class Extraction:
    start: date
    stop: date
    accounts: list[Account]

@dataclass
class Config:
    ssn: Optional[int]
    extractions: list[Extraction]

# The timeout in seconds for DOM elements to be found
Timeout = 5

def datestr_to_str(date_str: str) -> date:
    # Assumes that the format is "MM/YYYY"
    # The day will always be the first of the month for convenience
    day = 1
    month = int(date_str[:2])
    year = int(date_str[3:])

    return date(year, month, day)

def num_months(m1: date, m2: date):
    return (m1.year - m2.year) * 12 + m1.month - m2.month

def resolve_env():
    """ Adds the web drivers necessary for Selenium to work at runtime """
    
    basepath = f"{os.path.dirname(os.path.abspath(getsourcefile(lambda:0)))}/drivers"

    match sys.platform[:3]:
        case 'fre' | 'lin' | 'aix':
            # Linux
            os.environ['PATH'] += f":{basepath}/linux/"
        case 'dar':
            # Unix
            os.environ['PATH'] += f":{basepath}/macos/"
        case 'win':
            # Windows
            os.environ['PATH'] += f";{basepath}/windows/"

def find_firefox_exec():
    match sys.platform[:3]:
        case 'fre' | 'lin' | 'aix':
            # Firefox should exist in $PATH
            pass
        case 'dar':
            # Firefox should exist in $PATH
            pass
        case 'win':
            import winreg
            firefox_version = winreg.QueryValue(winreg.HKEY_LOCAL_MACHINE, "SOFTWARE\\Mozilla\\Mozilla Firefox")
            access_registry = winreg.ConnectRegistry(None,winreg.HKEY_LOCAL_MACHINE)
            key = winreg.OpenKey(access_registry,f"SOFTWARE\\Mozilla\\Mozilla Firefox {firefox_version}\\bin")
            return winreg.QueryValueEx(key, "PathToExe")[0]

def configure():
    """ Configures the driver with the correct options """
    conf = {}

    opt = webdriver.firefox.options.Options()
    opt.headless = True
    opt.binary = find_firefox_exec()
    prof = webdriver.FirefoxProfile()

    prof.set_preference('browser.download.folderList', 2)
    prof.set_preference('browser.download.manager.showWhenStarting', False)
    prof.set_preference('browser.download.dir', os.getcwd())
    prof.set_preference('browser.helperApps.neverAsk.saveToDisk', 'application/pdf')
    prof.set_preference('pdfjs.disabled', True)
    prof.set_preference('plugin.scan.plid.all', False)
    prof.set_preference('plugin.scan.Acrobat', "99.0")
    prof.set_preference('general.warnOnAboutConfig', False)
    prof.update_preferences()

    conf['firefox_profile'] = prof

    conf['options'] = opt

    return conf

def login(driver, ssn: Optional[int]):
    """ Navigates the user to DNB and logs them in using a PIN and OTP combo and waits for the content to load """

    print("Logging in")

    driver.get("https://dnb.no")

    try:
        WebDriverWait(driver, Timeout).until(EC.element_to_be_clickable((By.ID, 'consent-modal')))

        # Remove the modal block that may appear
        if driver.find_element_by_id('consent-modal').is_displayed():
            driver.find_element_by_id('consent-x').click()
    except TimeoutException:
        pass

    start_login_btn = driver.find_element_by_xpath("/html/body/div/div[1]/div[1]/section/header/div[2]/div/div/div[3]/div[2]/div/button")

    while not driver.find_elements(By.XPATH, '//form'):
        start_login_btn.click()

    # DNB has two stages of login
    # The first one is simply entering a user's SSN
    # Then the user has to select the login type
    form_1 = driver.find_element_by_xpath("//form")
    inp = form_1.find_element_by_xpath(".//input[@name='uid']")
    cnf = form_1.find_element_by_xpath(".//button[last()]")

    if not ssn:
        ssn = int(input("Please enter your SSN for DNB (11 digits): "))
    inp.clear()
    inp.send_keys(ssn)
    cnf.click()

    # Wait for the necessary DOM elements to be loaded
    WebDriverWait(driver, Timeout).until(EC.presence_of_element_located((By.ID, "r_state-2")))

    # Select the easier method of logging in and logging in
    nd_login = driver.find_element_by_xpath("//div[@id='r_state-2']")
    nd_login.find_element_by_xpath("./div[1]").click()

    # Locate all the neccesary fields to log in with a PIN and OTP combo
    form_2 = nd_login.find_element_by_xpath("./div[2]//form")
    pin = form_2.find_element_by_xpath(".//input[@id='phoneCode']")
    otp = form_2.find_element_by_xpath(".//input[@id='otpCode']")
    btn = form_2.find_element_by_xpath(".//button")

    # Clear the fields and ask for user input
    pin.clear()
    otp.clear()
    pin.send_keys(getpass("Please enter your PIN (4 digits): "))
    otp.send_keys(getpass("Please enter your one time password (6 digits): "))

    # Login
    btn.click()

    try:
        # Wait for the necessary DOM elements to be loaded
        WebDriverWait(driver, Timeout).until(EC.element_to_be_clickable((By.XPATH, "/html/body/div/div[1]/div/a")))

        # Force a navigation to the user's homepage
        logo_link = driver.find_element_by_xpath("//div[@id='logo']/a")
        logo_link.click()
    except TimeoutException:
        # Typically happens when the user isnt prompted with an intermidiate page
        pass

    # Wait for AJAX request to finish so that the required elements are present
    WebDriverWait(driver, Timeout).until(EC.presence_of_element_located((By.ID, "gllwg04e")))

def navigate(driver):
    """ navigate to the correct part of the DNB website """

    print("Navigating")

    top_menu = driver.find_element_by_xpath("//div[@id='menuLoggedIn']")
    m1 = top_menu.find_element_by_xpath(".//li[1]")

    # Activate the dropdown. May be optional
    m1.find_element_by_xpath("./a").click()

    # Locate the correct link
    m1.find_element_by_xpath(".//a[@id='gllwg07s']").click()

    WebDriverWait(driver, Timeout).until(EC.presence_of_element_located((By.ID, "documentType-button")))

    driver.execute_script('document.getElementById("documentType").style = "display: block;"')
    sel = Select(driver.find_element_by_xpath("//select[@id='documentType'] | //select[@name='documentType']"))
    sel.select_by_value('kontoutskrift')

def extract(driver, config: Config):
    """ Extract all the statements for the accounts given """
    print("Extracting")

    file_pattern = re.compile('(\\d{11})_-_(\\d{4}-\\d{2}).*')
    for extraction in config.extractions:
        start = num_months(date.today(), extraction.start)
        stop = num_months(date.today(), extraction.stop)
        months_ = list(range(start, stop, -1))

        for account in extraction.accounts:
            months = months_.copy()
            # Wait to ensure that the correct DOM elements are loaded
            WebDriverWait(driver, Timeout).until(EC.presence_of_element_located((By.ID, "documentType-button")))
            WebDriverWait(driver, Timeout).until(EC.presence_of_element_located((By.ID, "accountNumber")))

            # Select the correct account
            driver.execute_script('document.getElementById("accountNumber").style = "display: block;"')
            sel = Select(driver.find_element_by_xpath("//select[@id='accountNumber'] | //select[@name='accountNumber']"))
            sel.select_by_value(account.id.replace('.', ''))

            # Iterate over the given months
            # Goes until all the months have been extracted, even with timeouts
            while months:
                for month in months:
                    try:
                        WebDriverWait(driver, Timeout).until(EC.presence_of_element_located((By.ID, "searchIntervalIndex")))
                        driver.execute_script('document.getElementById("searchIntervalIndex").style = "display: block;"')
                        sel = Select(driver.find_element_by_xpath("//select[@id='searchIntervalIndex'] | //select[@name='searchIntervalIndex']"))
                        sel.select_by_value(f"{month}")

                        WebDriverWait(driver, Timeout).until(EC.presence_of_element_located((By.XPATH, "//input[@id='archiveSearchSubmit']")))
                        driver.find_element_by_xpath("//input[@id='archiveSearchSubmit']").click()
                        
                        try:
                            # Wait to ensure that the correct DOM elements are loaded
                            WebDriverWait(driver, Timeout).until(EC.presence_of_element_located((By.XPATH, "//table//a[@href='ajax/attachment/0/kontoutskrift'] | //div[@id='userInformationView']")))
                            # Click the file to download
                            driver.find_element_by_xpath("//table//a[@href='ajax/attachment/0/kontoutskrift']").click()
                        except NoSuchElementException:
                            # Inform the user if it's not possible to download
                            print(f"Could not find financial statement for {account} in {driver.find_element_by_id('searchIntervalIndex-button').text}")
                            months.remove(month)
                    except TimeoutException:
                        print(f"Timed out for {account} on {driver.find_element_by_id('searchIntervalIndex-button').text}")
                        pass

                for file in pathlib.Path(os.getcwd()).glob('*.pdf'):
                    match = file_pattern.search(file.stem)

                    if not match:
                        continue

                    # remove reference of the file if the file has been downloaded
                    if match.group(1) == account.id.replace('.', '') and (month := num_months(datetime.now(), datetime.strptime(match.group(2), "%Y-%m"))) in months:
                        months.remove(month)

            combine(account)

def combine(account: Account):
    """ Combines the downloaded pdfs into one and deletes the individual ones """

    basename = f"{account.name}" if account.name else f"{account.id}"

    print(f"Combining for {account}")

    file_pattern = re.compile(f"{account.id.replace('.', '')}_-_\\d{{4}}-\\d{{2}}-\\d{{2}}_-_Kontoutskrift")

    # Retrieve all the files pertaining to the account
    dl_path = pathlib.Path(os.getcwd())
    files = [x for x in dl_path.glob('*.pdf') if file_pattern.fullmatch(x.stem)]

    merger = PdfFileMerger()

    for file in sorted(files):
        merger.append(str(file))

    # Output the merged PDF
    merger.write(f"{basename}.pdf")
    merger.close()

def cleanup():
    """ A function who's whole point is to clean up files which may be missed in the combination step """

    print("Cleaning up remaining files")

    file_pattern = re.compile('(\\d{11})_-_(\\d{4}-\\d{2})(\\(\\d+\\))?.*')

    for file in pathlib.Path(os.getcwd()).glob('*.pdf'):
        match = file_pattern.search(file.stem)

        if match:
            file.unlink()

def main(argv):
    if len(argv) < 2:
        print("You need to add the path to the configuration file.")
        print("Usage: python main.py config.yaml")
        return

    old_cwd = os.getcwd()
    new_cwd = os.path.dirname(os.path.realpath(__file__))

    # Change directory to the directory of the current file
    os.chdir(new_cwd)

    config_path = os.path.abspath(os.path.join(old_cwd, sys.argv[1]))

    try:
        with open(config_path, 'r') as fi:
            config = from_dict(data_class=Config, data=yaml.load(fi.read(-1), Loader=Loader), config=DaConfig(
                type_hooks={
                    date: datestr_to_str
                }
            ))

    except Exception as e:
        print(e)
        print("Configuration file is probably incorrectly formatted. Please check the file.")
        if sys.platform.startswith('win'):
            input("Press enter to exit...")
        return

    resolve_env()

    # Instantiate the web browser and navigate to DNB
    driver = webdriver.Firefox(**configure())

    try:
        login(driver, config.ssn)
        navigate(driver)
        extract(driver, config)
        cleanup()
    except BaseException as e:
        log_timestamp = datetime.now().isoformat().replace("-", "_").replace(":", "_").replace(".", "_").replace("_", "")
        log_file = f"dnb_crawl_{log_timestamp}.log"
        log_path = os.path.abspath(os.path.join(new_cwd, log_file))
        with open(log_path, "w") as log_fi:
            log_fi.write(str(e))
            log_fi.write(traceback.format_exc())
        print(f"Exception occurred. Check {log_path} for why the exception occurred.")
        breakpoint()

    driver.quit()

    try:
        os.unlink(os.path.abspath(os.path.join(new_cwd, "geckodriver.log")))
    except BaseException as e:
        print(e)

if __name__ == '__main__':
    main(sys.argv)
