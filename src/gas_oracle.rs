use async_trait::async_trait;
use core::time::Duration;
use ethers::types::U256;
use reqwest::StatusCode;
use serde::Deserialize;
use std::fmt::Debug;
use tracing::trace;

use crate::transaction::Priority;

#[async_trait]
pub trait GasOracle: Debug {
    type Error: std::error::Error + Send + Sync;

    async fn gas_info(
        &self,
        priority: Priority,
    ) -> Result<GasInfo, Self::Error>;
}

#[derive(Debug, Clone, Copy)]
pub struct GasInfo {
    pub gas_price: U256, // in wei
    pub mining_time: Option<Duration>,
    pub block_time: Option<Duration>,
}

// Implementation using the ETH Gas Station API.

#[derive(Debug, thiserror::Error)]
pub enum GasOracleError {
    #[error("GET request error: {0}")]
    Request(reqwest::Error),

    #[error("invalid status code: {0}")]
    StatusCode(reqwest::StatusCode),

    #[error("could not parse the request's response: {0}")]
    ParseResponse(serde_json::Error),
}

#[derive(Debug)]
pub struct ETHGasStationOracle {
    api_key: String,
}

impl ETHGasStationOracle {
    pub fn new(api_key: String) -> ETHGasStationOracle {
        ETHGasStationOracle { api_key }
    }
}

#[async_trait]
impl GasOracle for ETHGasStationOracle {
    type Error = GasOracleError;

    #[tracing::instrument(level = "trace")]
    async fn gas_info(
        &self,
        priority: Priority,
    ) -> Result<GasInfo, Self::Error> {
        let url = format!(
            "https://ethgasstation.info/api/ethgasAPI.json?api-key={}",
            self.api_key
        );

        let res = reqwest::get(url).await.map_err(GasOracleError::Request)?;
        if res.status() != StatusCode::OK {
            return Err(GasOracleError::StatusCode(res.status()));
        }

        let bytes = &res.bytes().await.map_err(GasOracleError::Request)?;
        let response = serde_json::from_slice(bytes)
            .map_err(GasOracleError::ParseResponse)?;
        let gas_info = (response, priority).into();
        trace!("gas info: {:?}", gas_info);
        return Ok(gas_info);
    }
}

#[derive(Debug, Deserialize)]
struct ETHGasStationResponse {
    block_time: f32,
    fastest: u64,
    fast: u64,
    average: u64,
    #[serde(rename = "safeLow")]
    low: u64,
    #[serde(rename = "fastestWait")]
    fastest_time: f32,
    #[serde(rename = "fastWait")]
    fast_time: f32,
    #[serde(rename = "avgWait")]
    average_time: f32,
    #[serde(rename = "safeLowWait")]
    low_time: f32,
}

impl From<(ETHGasStationResponse, Priority)> for GasInfo {
    fn from((response, priority): (ETHGasStationResponse, Priority)) -> Self {
        let (gas_price, mining_time) = match priority {
            Priority::Low => (response.low, response.low_time),
            Priority::Normal => (response.average, response.average_time),
            Priority::High => (response.fast, response.fast_time),
            Priority::ASAP => (response.fastest, response.fastest_time),
        };

        // from 10*gwei to wei
        let mut gas_price = U256::from(gas_price);
        gas_price = gas_price.checked_mul(U256::exp10(10)).unwrap();

        GasInfo {
            gas_price,
            mining_time: Some(Duration::from_secs((mining_time * 60.) as u64)),
            block_time: Some(Duration::from_secs((response.block_time) as u64)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::gas_oracle::{ETHGasStationOracle, GasOracle};
    use crate::transaction::Priority;

    #[tokio::test]
    async fn test_eth_gas_station_oracle_ok() {
        // setup
        // tracing_subscriber::fmt::init();
        let gas_oracle = ETHGasStationOracle::new("works".to_string());

        // ok => priority low
        let result = gas_oracle.gas_info(Priority::Low).await;
        assert!(result.is_ok(), "{:?}", result);
        let gas_info_low = result.unwrap();

        // ok => priority normal
        let result = gas_oracle.gas_info(Priority::Normal).await;
        assert!(result.is_ok());
        let gas_info_normal = result.unwrap();

        // ok => priority high
        let result = gas_oracle.gas_info(Priority::High).await;
        assert!(result.is_ok());
        let gas_info_high = result.unwrap();

        // ok => priority ASAP
        let result = gas_oracle.gas_info(Priority::ASAP).await;
        assert!(result.is_ok());
        let gas_info_asap = result.unwrap();

        assert!(gas_info_low.gas_price <= gas_info_normal.gas_price);
        assert!(gas_info_normal.gas_price <= gas_info_high.gas_price);
        assert!(gas_info_high.gas_price <= gas_info_asap.gas_price);
    }

    #[tokio::test]
    async fn test_eth_gas_station_oracle_invalid_api_key() {
        // setup
        let invalid1 = ETHGasStationOracle::new("invalid".to_string());
        let invalid2 = ETHGasStationOracle::new("".to_string());

        // ok => invalid API key works (for some reason)
        let result = invalid1.gas_info(Priority::Normal).await;
        assert!(result.is_ok());

        // ok => empty API key works (for some reason)
        let result = invalid2.gas_info(Priority::Normal).await;
        assert!(result.is_ok());
    }
}
