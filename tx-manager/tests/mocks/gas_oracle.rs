use anyhow::{anyhow, Result};
use async_trait::async_trait;

use tx_manager::gas_oracle::GasInfo;
use tx_manager::transaction::Priority;

#[derive(Debug, thiserror::Error)]
pub enum GasOracleError {
    #[error("gas oracle mock error: gas info")]
    GasInfo,
}

#[derive(Debug)]
pub struct GasOracle {
    pub gas_info_output: Option<GasInfo>,
}

impl GasOracle {
    pub fn new() -> Self {
        Self {
            gas_info_output: None,
        }
    }
}

#[async_trait]
impl tx_manager::gas_oracle::GasOracle for GasOracle {
    async fn gas_info(&self, _: Priority) -> Result<GasInfo> {
        self.gas_info_output
            .ok_or(anyhow!(GasOracleError::GasInfo))
            .map(|x| x.clone())
    }
}
