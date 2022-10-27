use async_trait::async_trait;
use core::time::Duration;
use ethers::types::U256;
use std::error::Error;
use std::fmt::Debug;

use crate::transaction::Priority;

#[async_trait]
pub trait GasOracle: Debug {
    type Error: Error + Send + Sync;

    async fn get_info(&self, priority: Priority) -> Result<GasOracleInfo, Self::Error>;
}

#[derive(Debug, Clone, Copy)]
pub struct GasOracleInfo {
    pub gas_info: GasInfo,
    pub mining_time: Option<Duration>,
    pub block_time: Option<Duration>,
}

#[derive(Debug, Clone, Copy)]
pub enum GasInfo {
    Legacy(LegacyGasInfo),
    EIP1559(EIP1559GasInfo),
}

impl GasInfo {
    pub fn is_legacy(&self) -> bool {
        matches!(self, GasInfo::Legacy(_))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LegacyGasInfo {
    pub gas_price: U256,
}

#[derive(Debug, Clone, Copy)]
pub struct EIP1559GasInfo {
    pub max_fee: U256,
    pub max_priority_fee: Option<U256>,
}

impl TryFrom<GasInfo> for LegacyGasInfo {
    type Error = &'static str;

    fn try_from(gas_info: GasInfo) -> Result<Self, Self::Error> {
        match gas_info {
            GasInfo::Legacy(legacy_gas_info) => Ok(legacy_gas_info),
            GasInfo::EIP1559(_) => Err("expected legacy gas info, got EIP1559 gas info"),
        }
    }
}

impl TryFrom<GasInfo> for EIP1559GasInfo {
    type Error = &'static str;

    fn try_from(gas_info: GasInfo) -> Result<Self, Self::Error> {
        match gas_info {
            GasInfo::Legacy(_) => Err("expected EIP1559 gas info, got legacy gas info".into()),
            GasInfo::EIP1559(eip1559_gas_info) => Ok(eip1559_gas_info),
        }
    }
}
