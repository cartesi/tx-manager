use anyhow::{bail, Result};
use async_trait::async_trait;

use tx_manager::gas_oracle::GasInfo;
use tx_manager::transaction::Priority;

#[derive(Debug, thiserror::Error)]
pub enum GasOracleOutput {
    #[error("gas oracle mock output: gas info ok -- {0:?}")]
    GasInfoOk(GasInfo),

    #[error("gas oracle mock output: gas info error")]
    GasInfoError,

    #[error("gas oracle mock output: unreachable error")]
    Unreachable,
}

pub struct GasOracle {
    pub output: GasOracleOutput,
}

#[async_trait]
impl tx_manager::gas_oracle::GasOracle for GasOracle {
    async fn gas_info(&self, _: Priority) -> Result<GasInfo> {
        match self.output {
            GasOracleOutput::GasInfoOk(gas_info) => Ok(gas_info),
            GasOracleOutput::GasInfoError => {
                bail!(GasOracleOutput::GasInfoError)
            }
            _ => bail!(GasOracleOutput::Unreachable),
        }
    }
}
