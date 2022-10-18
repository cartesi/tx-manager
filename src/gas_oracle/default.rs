use async_trait::async_trait;
use ethers::providers::Middleware;

use std::fmt::Debug;

use ethers::types::Chain;

use crate::gas_oracle::{EIP1559GasInfo, GasInfo, GasOracle, GasOracleInfo, LegacyGasInfo};
use crate::transaction::Priority;

#[derive(Debug)]
pub struct DefaultGasOracle {
    chain: Chain,
}

impl DefaultGasOracle {
    pub fn new(chain: Chain) -> DefaultGasOracle {
        DefaultGasOracle { chain }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DefaultGasOracleError {
    #[error("get gas price: {0}")]
    GetGasPrice(String),

    #[error("estimate EIP1559 fees: {0}")]
    EstimateEIP1559Fees(String),
}

#[async_trait]
impl GasOracle for DefaultGasOracle {
    type Error = DefaultGasOracleError;

    async fn get_info<M: Middleware>(
        &self,
        _: Priority,
        provider: &M,
    ) -> Result<GasOracleInfo, Self::Error> {
        let gas_info = if self.chain.is_legacy() {
            let gas_price = provider
                .get_gas_price()
                .await
                .map_err(|err| DefaultGasOracleError::GetGasPrice(err.to_string()))?;
            GasInfo::Legacy(LegacyGasInfo { gas_price })
        } else {
            let (max_fee, max_priority_fee) = provider
                .estimate_eip1559_fees(None)
                .await
                .map_err(|err| DefaultGasOracleError::GetGasPrice(err.to_string()))?;
            GasInfo::EIP1559(EIP1559GasInfo {
                max_fee,
                max_priority_fee: Some(max_priority_fee),
            })
        };
        Ok(GasOracleInfo {
            gas_info,
            mining_time: None,
            block_time: None,
        })
    }
}
