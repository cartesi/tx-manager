use anyhow::{bail, Result};
use async_trait::async_trait;

use tx_manager::gas_oracle::GasInfo;
use tx_manager::transaction::Priority;

#[derive(Debug, thiserror::Error)]
pub enum GasOracleError {
    #[error("gas oracle mock output: gas info error")]
    GasInfoError,
}

pub struct GasOracle {
    pub gas_info: (bool, Option<GasInfo>),
}

impl GasOracle {
    pub fn new() -> Self {
        Self {
            gas_info: (false, None),
        }
    }

    pub fn reset(&mut self) {
        self.gas_info = (false, None);
    }
}

#[async_trait]
impl tx_manager::gas_oracle::GasOracle for GasOracle {
    async fn gas_info(&self, _: Priority) -> Result<GasInfo> {
        if self.gas_info.0 {
            Ok(self.gas_info.1.unwrap())
        } else {
            bail!(GasOracleError::GasInfoError)
        }
    }
}
