use clap::Parser;
use std::fmt::Debug;

use crate::Chain;

#[derive(Clone, Parser)]
#[command(name = "tx_config")]
#[command(about = "Configuration for transaction manager")]
pub struct TxEnvCLIConfig {
    /// Blockchain provider http endpoint url
    #[arg(long, env)]
    pub tx_provider_http_endpoint: Option<String>,

    /// Chain ID
    #[arg(long, env)]
    pub tx_chain_id: Option<u64>,

    /// EIP1559 flag
    #[arg(long, env)]
    pub tx_chain_is_legacy: Option<bool>,

    /// Path to tx-manager database file
    #[arg(long, env)]
    pub tx_database_path: Option<String>,

    /// Ethereum gas station oracle api key
    #[arg(long, env)]
    pub tx_gas_oracle_api_key: Option<String>,

    /// Default confirmations
    #[arg(long, env)]
    pub tx_default_confirmations: Option<usize>,
}

#[derive(Clone)]
pub struct TxManagerConfig {
    pub default_confirmations: usize,
    pub provider_http_endpoint: String,
    pub chain_id: u64,
    pub chain_is_legacy: bool,
    pub database_path: String,
    pub gas_oracle_api_key: String,
}

impl Debug for TxManagerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxManagerConfig")
            .field("default_confirmations", &self.default_confirmations)
            .field("provider_http_endpoint", &self.provider_http_endpoint)
            .field("chain_id", &self.chain_id)
            .field("chain_is_legacy", &self.chain_is_legacy)
            .field("database_path", &self.database_path)
            .field("gas_oracle_api_key", &self.gas_oracle_api_key)
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Configuration missing chain_id")]
    MissingChainId,
}

pub type Result<T> = std::result::Result<T, Error>;

const DEFAULT_DEFAULT_CONFIRMATIONS: usize = 7;
const DEFAULT_HTTP_ENDPOINT: &str = "http://localhost:8545";
const DEFAULT_DATABASE_PATH: &str = "./default_tx_database";
const DEFAULT_GAS_ORACLE_API_KEY: &str = "";

impl TxManagerConfig {
    pub fn initialize_from_args() -> Result<Self> {
        let env_cli_config = TxEnvCLIConfig::parse();
        Self::initialize(env_cli_config)
    }

    pub fn initialize(env_cli_config: TxEnvCLIConfig) -> Result<Self> {
        let default_confirmations = env_cli_config
            .tx_default_confirmations
            .unwrap_or(DEFAULT_DEFAULT_CONFIRMATIONS);

        let provider_http_endpoint = env_cli_config
            .tx_provider_http_endpoint
            .unwrap_or_else(|| DEFAULT_HTTP_ENDPOINT.to_string());

        let chain_id = env_cli_config.tx_chain_id.ok_or(Error::MissingChainId)?;
        let chain_is_legacy = env_cli_config.tx_chain_is_legacy.unwrap_or(false);

        let database_path = env_cli_config
            .tx_database_path
            .unwrap_or_else(|| DEFAULT_DATABASE_PATH.to_string());

        let gas_oracle_api_key = env_cli_config
            .tx_gas_oracle_api_key
            .unwrap_or_else(|| DEFAULT_GAS_ORACLE_API_KEY.to_string());

        Ok(Self {
            default_confirmations,
            provider_http_endpoint,
            chain_id,
            chain_is_legacy,
            database_path,
            gas_oracle_api_key,
        })
    }
}

impl From<&TxManagerConfig> for Chain {
    fn from(config: &TxManagerConfig) -> Self {
        Self {
            id: config.chain_id,
            is_legacy: config.chain_is_legacy,
        }
    }
}
