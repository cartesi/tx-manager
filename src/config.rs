use offchain_utils::configuration;

use configuration::error as config_error;

use serde::Deserialize;
use std::time::Duration;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(
    name = "tm_config",
    about = "Configuration for transaction manager"
)]
struct TMEnvCLIConfig {
    /// Path to transaction manager config
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
    pub tm_max_delay: Option<u64>,
    pub tm_max_retries: Option<usize>,
    pub tm_timeout: Option<u64>,
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
    pub fn initialize() -> config_error::Result<Self> {
        let env_cli_config = TMEnvCLIConfig::from_args();

        let file_config: TMFileConfig =
            configuration::config::load_config_file(
                env_cli_config.tm_config,
                "tx-manager",
            )?;

        let max_delay = Duration::from_secs(
            env_cli_config
                .tm_max_delay
                .or(file_config.tm_max_delay)
                .unwrap_or(DEFAULT_MAX_DELAY),
        );

        let max_retries = env_cli_config
            .tm_max_retries
            .or(file_config.tm_max_retries)
            .unwrap_or(DEFAULT_MAX_RETRIES);

        let transaction_timeout = Duration::from_secs(
            env_cli_config
                .tm_timeout
                .or(file_config.tm_timeout)
                .unwrap_or(DEFAULT_TIMEOUT),
        );

        Ok(TMConfig {
            max_delay,
            max_retries,
            transaction_timeout,
        })
    }
}
