use ethers::types::Address;
use snafu::{ResultExt, Snafu};
use std::fs;
use structopt::StructOpt;

#[derive(StructOpt, Clone)]
#[structopt(
    name = "tx_config",
    about = "Configuration for transaction manager"
)]
pub struct TxEnvCLIConfig {
    /// Blockchain provider http endpoint url
    #[structopt(long)]
    pub tx_provider_http_endpoint: Option<String>,

    /// Signer mnemonic, overrides `tx_mnemonic_file`
    #[structopt(long)]
    pub tx_mnemonic: Option<String>,

    /// Signer mnemonic file path
    #[structopt(long)]
    pub tx_mnemonic_file: Option<String>,

    /// Signer public address
    #[structopt(long)]
    pub tx_sender: Option<Address>,

    /// Chain ID
    #[structopt(long)]
    pub tx_chain_id: Option<u64>,

    /// Path to tx manager database file
    #[structopt(long)]
    pub tx_database_path: Option<String>,

    /// Ethereum gas station oracle api key
    #[structopt(long)]
    pub tx_gas_oracle_api_key: Option<String>,

    /// Default confirmations
    #[structopt(long)]
    pub tx_default_confirmations: Option<usize>,
}

#[derive(Clone)]
pub struct TxManagerConfig {
    pub default_confirmations: usize,
    pub provider_http_endpoint: String,
    pub mnemonic: String,
    pub chain_id: u64,
    pub sender: Address,
    pub database_path: String,
    pub gas_oracle_api_key: String,
}

impl std::fmt::Debug for TxManagerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxManagerConfig")
            .field("default_confirmations", &self.default_confirmations)
            .field("provider_http_endpoint", &self.provider_http_endpoint)
            .field("mnemonic", &"REDACTED")
            .field("chain_id", &self.chain_id)
            .field("sender", &self.sender)
            .field("database_path", &self.database_path)
            .field("gas_oracle_api_key", &self.gas_oracle_api_key)
            .finish()
    }
}

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Configuration missing mnemonic"))]
    MissingMnemonic {},

    #[snafu(display(
        "Could not read mnemonic file at path `{}`: {}",
        path,
        source
    ))]
    MnemonicFileReadError {
        path: String,
        source: std::io::Error,
    },

    #[snafu(display("Configuration missing sender address"))]
    MissingSender {},

    #[snafu(display("Configuration missing chain_id"))]
    MissingChainId {},
}

pub type Result<T> = std::result::Result<T, Error>;

const DEFAULT_DEFAULT_CONFIRMATIONS: usize = 7;
const DEFAULT_HTTP_ENDPOINT: &str = "http://localhost:8545";
const DEFAULT_DATABASE_PATH: &str = "./default_tx_database";
const DEFAULT_GAS_ORACLE_API_KEY: &str = "";

impl TxManagerConfig {
    pub fn initialize_from_args() -> Result<Self> {
        let env_cli_config = TxEnvCLIConfig::from_args();
        Self::initialize(env_cli_config)
    }

    pub fn initialize(env_cli_config: TxEnvCLIConfig) -> Result<Self> {
        let default_confirmations = env_cli_config
            .tx_default_confirmations
            .unwrap_or(DEFAULT_DEFAULT_CONFIRMATIONS);

        let provider_http_endpoint = env_cli_config
            .tx_provider_http_endpoint
            .unwrap_or_else(|| DEFAULT_HTTP_ENDPOINT.to_string());

        let mnemonic: String = if let Some(m) = env_cli_config.tx_mnemonic {
            m
        } else {
            let path = env_cli_config
                .tx_mnemonic_file
                .ok_or(snafu::NoneError)
                .context(MissingMnemonicSnafu)?;

            let contents = fs::read_to_string(path.clone())
                .context(MnemonicFileReadSnafu { path })?;

            contents.trim().to_string()
        };

        let chain_id = env_cli_config
            .tx_chain_id
            .ok_or(snafu::NoneError)
            .context(MissingChainIdSnafu)?;

        let sender = env_cli_config
            .tx_sender
            .ok_or(snafu::NoneError)
            .context(MissingSenderSnafu)?;

        let database_path = env_cli_config
            .tx_database_path
            .unwrap_or_else(|| DEFAULT_DATABASE_PATH.to_string());

        let gas_oracle_api_key = env_cli_config
            .tx_gas_oracle_api_key
            .unwrap_or_else(|| DEFAULT_GAS_ORACLE_API_KEY.to_string());

        Ok(Self {
            default_confirmations,
            provider_http_endpoint,
            mnemonic,
            chain_id,
            sender,
            database_path,
            gas_oracle_api_key,
        })
    }
}
