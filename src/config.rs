use offchain_utils::configuration;

use configuration::error as config_error;

use serde::Deserialize;
use std::time::Duration;
use structopt::StructOpt;

#[derive(StructOpt, Clone, Debug)]
#[structopt(
    name = "tm_config",
    about = "Configuration for transaction manager"
)]
pub struct TMEnvCLIConfig {
    /// Path to transaction manager .toml config
    #[structopt(long, env)]
    pub tm_config: Option<String>,
    /// Max delay (secs) between retries
    #[structopt(long, env)]
    pub tm_max_delay: Option<u64>,
    /// Max retries for a transaction
    #[structopt(long, env)]
    pub tm_max_retries: Option<usize>,
    /// Timeout value (secs) for a transaction
    #[structopt(long, env)]
    pub tm_timeout: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct TMFileConfig {
    pub max_delay: Option<u64>,
    pub max_retries: Option<usize>,
    pub timeout: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct FileConfig {
    pub tx_manager: TMFileConfig,
}

#[derive(Clone, Debug)]
pub struct TMConfig {
    pub max_delay: Duration,
    pub max_retries: usize,
    pub transaction_timeout: Duration,
}

// default values
const DEFAULT_MAX_DELAY: u64 = 1;
const DEFAULT_MAX_RETRIES: usize = 5;
const DEFAULT_TIMEOUT: u64 = 5;

impl TMConfig {
    pub fn initialize(
        env_cli_config: TMEnvCLIConfig,
    ) -> config_error::Result<Self> {
        let file_config: FileConfig =
            configuration::config::load_config_file(env_cli_config.tm_config)?;

        let max_delay = Duration::from_secs(
            env_cli_config
                .tm_max_delay
                .or(file_config.tx_manager.max_delay)
                .unwrap_or(DEFAULT_MAX_DELAY),
        );

        let max_retries = env_cli_config
            .tm_max_retries
            .or(file_config.tx_manager.max_retries)
            .unwrap_or(DEFAULT_MAX_RETRIES);

        let transaction_timeout = Duration::from_secs(
            env_cli_config
                .tm_timeout
                .or(file_config.tx_manager.timeout)
                .unwrap_or(DEFAULT_TIMEOUT),
        );

        Ok(TMConfig {
            max_delay,
            max_retries,
            transaction_timeout,
        })
    }
}
