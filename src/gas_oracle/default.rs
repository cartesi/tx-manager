use async_trait::async_trait;
use std::fmt::Debug;

use crate::gas_oracle::{GasOracle, GasOracleInfo};
use crate::transaction::Priority;

#[derive(Debug, thiserror::Error)]
pub enum DefaultGasOracleError {
    #[error("defaulting")]
    Default,
}

#[derive(Clone, Debug, Default)]
pub struct DefaultGasOracle {}

impl DefaultGasOracle {
    pub fn new() -> DefaultGasOracle {
        DefaultGasOracle::default()
    }
}

#[async_trait]
impl GasOracle for DefaultGasOracle {
    type Error = DefaultGasOracleError;

    async fn get_info(&self, _: Priority) -> Result<GasOracleInfo, Self::Error> {
        Err(DefaultGasOracleError::Default)
    }
}
