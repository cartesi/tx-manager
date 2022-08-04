use async_trait::async_trait;
use core::time::Duration;
use ethers::types::U256;
use std::error::Error;
use std::fmt::Debug;

use crate::transaction::Priority;
#[async_trait]
pub trait GasOracle: Debug {
    type Error: Error + Send + Sync;

    async fn gas_info(&self, priority: Priority) -> Result<GasInfo, Self::Error>;
}

#[derive(Debug, Clone, Copy)]
pub struct GasInfo {
    pub max_fee: U256, // in wei for ethereum
    pub max_priority_fee: Option<U256>,
    pub mining_time: Option<Duration>,
    pub block_time: Option<Duration>,
}
