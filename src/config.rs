use ethers::signers::{coins_bip39::English, LocalWallet, MnemonicBuilder, Signer, WalletError};
use std::fs;
use structopt::StructOpt;

use crate::Chain;

#[derive(StructOpt, Clone)]
#[structopt(name = "tx_config", about = "Configuration for transaction manager")]
pub struct TxEnvCLIConfig {
    /// Blockchain provider http endpoint url
    #[structopt(long, env)]
    pub tx_provider_http_endpoint: Option<String>,

    /// Signer mnemonic, overrides `tx_mnemonic_file`
    #[structopt(long, env)]
    pub tx_mnemonic: Option<String>,

    /// Signer mnemonic file path
    #[structopt(long, env)]
    pub tx_mnemonic_file: Option<String>,

    /// Mnemonic account index
    #[structopt(long, env)]
    pub tx_mnemonic_account_index: Option<u32>,

    /// Chain ID
    #[structopt(long, env)]
    pub tx_chain_id: Option<u64>,

    /// EIP1559 flag
    #[structopt(long, env)]
    pub tx_chain_is_legacy: Option<bool>,

    /// Path to tx manager database file
    #[structopt(long, env)]
    pub tx_database_path: Option<String>,

    /// Ethereum gas station oracle api key
    #[structopt(long, env)]
    pub tx_gas_oracle_api_key: Option<String>,

    /// Default confirmations
    #[structopt(long, env)]
    pub tx_default_confirmations: Option<usize>,
}

#[derive(Clone)]
pub struct TxManagerConfig {
    pub default_confirmations: usize,
    pub provider_http_endpoint: String,
    pub wallet: LocalWallet,
    pub chain_id: u64,
    pub chain_is_legacy: bool,
    pub database_path: String,
    pub gas_oracle_api_key: String,
}

impl std::fmt::Debug for TxManagerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxManagerConfig")
            .field("default_confirmations", &self.default_confirmations)
            .field("provider_http_endpoint", &self.provider_http_endpoint)
            .field("chain_id", &self.chain_id)
            .field("chain_is_legacy", &self.chain_is_legacy)
            .field("wallet_address", &self.wallet.address())
            .field("database_path", &self.database_path)
            .field("gas_oracle_api_key", &self.gas_oracle_api_key)
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Configuration missing chain_id")]
    MissingChainId,

    #[error("Configuration missing chain_is_legacy")]
    MissingChainIsLegacy,

    #[error("Configuration missing mnemonic")]
    MissingMnemonic,

    #[error("Could not read mnemonic file at path `{}`: {}", path, source)]
    MnemonicFileReadError {
        path: String,
        source: std::io::Error,
    },

    #[error("Mnemonic index malformed: {:?}", source)]
    MnemonicIndexMalformed { source: WalletError },

    #[error("Mnemonic malformed: {0}")]
    MnemonicMalformed(WalletError),
}

pub type Result<T> = std::result::Result<T, Error>;

const DEFAULT_MNEMONIC_ACCOUNT_INDEX: u32 = 0;
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

        let chain_id = env_cli_config.tx_chain_id.ok_or(Error::MissingChainId)?;

        let chain_is_legacy = env_cli_config
            .tx_chain_is_legacy
            .ok_or(Error::MissingChainIsLegacy)?;

        let wallet = {
            let mnemonic: String = if let Some(m) = env_cli_config.tx_mnemonic {
                m
            } else {
                let path = env_cli_config
                    .tx_mnemonic_file
                    .ok_or(Error::MissingMnemonic)?;

                let contents = fs::read_to_string(path.clone())
                    .map_err(|source| Error::MnemonicFileReadError { path, source })?;

                contents.trim().to_string()
            };

            let index = env_cli_config
                .tx_mnemonic_account_index
                .unwrap_or(DEFAULT_MNEMONIC_ACCOUNT_INDEX);

            MnemonicBuilder::<English>::default()
                .phrase(mnemonic.as_str())
                .index(index)
                .map_err(Error::MnemonicMalformed)?
                .build()
                .map_err(Error::MnemonicMalformed)?
                .with_chain_id(chain_id)
        };

        let database_path = env_cli_config
            .tx_database_path
            .unwrap_or_else(|| DEFAULT_DATABASE_PATH.to_string());

        let gas_oracle_api_key = env_cli_config
            .tx_gas_oracle_api_key
            .unwrap_or_else(|| DEFAULT_GAS_ORACLE_API_KEY.to_string());

        Ok(Self {
            default_confirmations,
            provider_http_endpoint,
            wallet,
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
