use std::fs::File;
use std::path::PathBuf;

use chrono::naive::NaiveDate;
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub ssn: Option<String>,
    pub extractions: Vec<Extraction>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct Extraction {
    #[serde(with = "date_formatter")]
    pub from: NaiveDate,

    #[serde(with = "date_formatter")]
    pub to: NaiveDate,

    pub accounts: Vec<Account>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct Account {
    pub id: String,
    pub name: Option<String>,
}

pub fn read_config(path: &PathBuf) -> Result<Config, String> {
    if !path.exists() {
        return Err("Given configuration does not exist".to_string());
    }

    let file = File::open(path).expect("Unable to open given configuration file");
    Ok(serde_yaml::from_reader(file).unwrap())
}

mod date_formatter {
    use super::*;
    use serde::Deserializer;

    const FORMAT: &str = "%d/%m/%Y";

    pub fn deserialize<'de, D>(deserializer: D) -> Result<NaiveDate, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::from("01/") + String::deserialize(deserializer)?.as_str();
        Ok(NaiveDate::parse_from_str(&s, FORMAT).unwrap())
    }
}

#[cfg(test)]
mod parsing {
    use super::*;

    #[test]
    fn basic() {
        let config_str = "
ssn: 00000000000
extractions:
  - from: 01/2020
    to: 01/2021
    accounts:
    - id: 1234.56.78901
      name: test
";

        let config = Config {
            ssn: Some("00000000000".to_string()),
            extractions: vec![Extraction {
                from: NaiveDate::from_ymd_opt(2020, 01, 1).unwrap(),
                to: NaiveDate::from_ymd_opt(2021, 01, 1).unwrap(),
                accounts: vec![Account {
                    id: "1234.56.78901".to_string(),
                    name: Some("test".to_string()),
                }],
            }],
        };

        let parsed_config: Config = serde_yaml::from_str(config_str).unwrap();

        assert_eq!(parsed_config, config);
    }

    #[test]
    fn multiple_accounts() {
        let config_str = "
ssn: 00000000000
extractions:
  - from: 01/2020
    to: 01/2021
    accounts:
    - id: 1234.56.78901
      name: test
    - id: 1234.00.78901
      name: test2
";

        let config = Config {
            ssn: Some("00000000000".to_string()),
            extractions: vec![Extraction {
                from: NaiveDate::from_ymd_opt(2020, 01, 1).unwrap(),
                to: NaiveDate::from_ymd_opt(2021, 01, 1).unwrap(),
                accounts: vec![
                    Account {
                        id: "1234.56.78901".to_string(),
                        name: Some("test".to_string()),
                    },
                    Account {
                        id: "1234.00.78901".to_string(),
                        name: Some("test2".to_string()),
                    },
                ],
            }],
        };

        let parsed_config: Config = serde_yaml::from_str(config_str).unwrap();

        assert_eq!(parsed_config, config);
    }

    #[test]
    fn multiple_extractions() {
        let config_str = "
ssn: 00000000000
extractions:
  - from: 01/2020
    to: 01/2021
    accounts:
    - id: 1234.56.78901
      name: test-2020
  - from: 01/2021
    to: 01/2022
    accounts:
    - id: 1234.56.78901
      name: test-2021
";

        let config = Config {
            ssn: Some("00000000000".to_string()),
            extractions: vec![
                Extraction {
                    from: NaiveDate::from_ymd_opt(2020, 01, 1).unwrap(),
                    to: NaiveDate::from_ymd_opt(2021, 01, 1).unwrap(),
                    accounts: vec![Account {
                        id: "1234.56.78901".to_string(),
                        name: Some("test-2020".to_string()),
                    }],
                },
                Extraction {
                    from: NaiveDate::from_ymd_opt(2021, 01, 1).unwrap(),
                    to: NaiveDate::from_ymd_opt(2022, 01, 1).unwrap(),
                    accounts: vec![Account {
                        id: "1234.56.78901".to_string(),
                        name: Some("test-2021".to_string()),
                    }],
                },
            ],
        };

        let parsed_config: Config = serde_yaml::from_str(config_str).unwrap();

        assert_eq!(parsed_config, config);
    }
}
