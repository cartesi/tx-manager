use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, fs};

use crate::Account;

pub const TEST_CONFIGURATION_PATH: &str = "tests/configuration.json";

#[derive(Serialize, Deserialize)]
pub struct Configuration {
    pub provider_http_url: HashMap<String, String>,
    pub account1: Account,
    pub account2: Account,
}

impl Configuration {
    pub fn get(path: String) -> Self {
        if let Ok(s) = fs::read_to_string(path) {
            return serde_json::from_str(&s).unwrap();
        } else {
            let keys = vec!["ethereum", "arbitrum", "optimism", "polygon"];

            let mut map = HashMap::new();
            for key in keys.iter() {
                let key = key.to_string();
                let env_var = key.to_uppercase() + "_URL";
                let url = env::var(env_var.clone());
                assert!(url.is_ok(), "missing {}", env_var);
                map.insert(key, url.unwrap());
            }

            Configuration {
                provider_http_url: map,
                account1: Self::read_account_from_env(1),
                account2: Self::read_account_from_env(2),
            }
        }
    }

    /// Auxiliary.
    fn read_account_from_env(n: u8) -> Account {
        let s = "ACCOUNT".to_string() + &n.to_string();

        let address_str = s.clone() + "_ADDRESS";
        let private_key_str = s.clone() + "_PRIVATE_KEY";

        let address = env::var(address_str.clone());
        let private_key = env::var(private_key_str.clone());

        assert!(address.is_ok(), "missing {:?}", address_str);
        assert!(private_key.is_ok(), "missing {:?}", private_key_str);

        Account {
            address: address.unwrap(),
            private_key: private_key.unwrap(),
        }
    }
}
