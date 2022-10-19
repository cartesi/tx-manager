use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs};

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
        let s = fs::read_to_string(path).unwrap();
        let configuration: Configuration = serde_json::from_str(&s).unwrap();
        configuration
    }
}
