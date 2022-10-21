use async_trait::async_trait;
use ethers::types::U256;
use tx_manager::{
    gas_oracle::{EIP1559GasInfo, GasInfo, GasOracle, GasOracleInfo},
    transaction::Priority,
};

/// Always returns a 2 gwei max fee and 1 gwei max priority fee.
#[derive(Debug)]
pub struct ConstantGasOracle {}

impl ConstantGasOracle {
    pub fn new() -> Self {
        Self {}
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConstantGasOracleError {}

#[async_trait]
impl GasOracle for ConstantGasOracle {
    type Error = ConstantGasOracleError;

    async fn get_info(&self, _: Priority) -> Result<GasOracleInfo, Self::Error> {
        Ok(GasOracleInfo {
            gas_info: GasInfo::EIP1559(EIP1559GasInfo {
                max_fee: U256::from(2e9 as u32),
                max_priority_fee: Some(U256::from(1e9 as u32)),
            }),
            mining_time: None,
            block_time: None,
        })
    }
}
