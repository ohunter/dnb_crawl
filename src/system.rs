use std::path::PathBuf;

pub fn find_firefox() -> Result<PathBuf, String> {
    locate::firefox()
}

#[cfg(windows)]
mod locate {
    use super::*;
    extern crate winreg;
    use winreg::enums::*;
    use winreg::RegKey;

    pub fn firefox() -> Result<PathBuf, String> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

        let cur_ver: String = match hklm.open_subkey("SOFTWARE\\Mozilla\\Mozilla Firefox") {
            Ok(val) => val.get_value("CurrentVersion").unwrap(),
            Err(err) => {
                return Err(format!(
                    "Unable to locate registry keys for Mozilla Firefox: {err}"
                ));
            }
        };

        match hklm.open_subkey(format!(
            "SOFTWARE\\Mozilla\\Mozilla Firefox\\{cur_ver}\\Main"
        )) {
            Ok(val) => {
                let tmp: String = val.get_value("PathToExe").unwrap();
                Ok(tmp.into())
            }
            Err(err) => Err(format!("Unable to locate firefox: {err}")),
        }
    }
}

#[cfg(unix)]
mod locate {
    use super::*;
    use std::env;

    pub fn firefox() -> Result<PathBuf, String> {
        !unimplemented!("Need to implement a method for finding the firefox exec on linux")
    }
}
