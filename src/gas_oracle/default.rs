use async_trait::async_trait;
use ethers::providers::Middleware;

use std::fmt::Debug;

use ethers::types::Chain;

use crate::gas_oracle::{EIP1559GasInfo, GasInfo, GasOracle, GasOracleInfo, LegacyGasInfo};
use crate::transaction::Priority;

#[derive(Debug)]
pub struct DefaultGasOracle {
}

impl DefaultGasOracle {
    pub fn new() -> DefaultGasOracle {
        DefaultGasOracle {}
    }
}

#[async_trait]
impl GasOracle for DefaultGasOracle {
    type Error = ();

    async fn get_info<M: Middleware>(
        &self,
        _: Priority
    ) -> Result<GasOracleInfo, Self::Error> {
        Err(())
    }
}
