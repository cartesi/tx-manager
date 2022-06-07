use anyhow::{bail, Result};
use async_trait::async_trait;
use core::time::Duration;
use ethers::types::U256;
use reqwest::StatusCode;
use serde::Deserialize;
use std::fmt::Debug;
use tracing::info;

use crate::transaction::Priority;

#[async_trait]
pub trait GasOracle: Debug {
    async fn gas_info(&self, priority: Priority) -> Result<GasInfo>;
}

#[derive(Debug, Clone, Copy)]
pub struct GasInfo {
    pub gas_price: U256, // in wei
    pub mining_time: Option<Duration>,
    pub block_time: Option<Duration>,
}

// Implementation using the ETH Gas Station API.

#[derive(Debug)]
pub struct ETHGasStationOracle {
    api_key: &'static str,
}

impl ETHGasStationOracle {
    pub fn new(api_key: &'static str) -> ETHGasStationOracle {
        return ETHGasStationOracle { api_key };
    }
}

#[async_trait]
impl GasOracle for ETHGasStationOracle {
    #[tracing::instrument]
    async fn gas_info(&self, priority: Priority) -> Result<GasInfo> {
        let url = format!(
            "https://ethgasstation.info/api/ethgasAPI.json?api-key={}",
            self.api_key
        );

        let res = reqwest::get(url).await?;
        if res.status() != StatusCode::OK {
            bail!("invalid status code: {}", res.status());
        }

        let response = serde_json::from_slice(&res.bytes().await?)?;
        let gas_info = (response, priority).into();
        info!("gas info: {:?}", gas_info);
        return Ok(gas_info);
    }
}

#[derive(Debug, Deserialize)]
struct ETHGasStationResponse {
    block_time: f32,
    fastest: String,
    fast: String,
    average: String,
    #[serde(rename = "safeLow")]
    low: String,
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
        let mut gas_price = U256::from_dec_str(&gas_price.to_string()).unwrap();
        gas_price = gas_price.checked_mul(U256::exp10(10)).unwrap();

        return GasInfo {
            gas_price,
            mining_time: Some(Duration::from_secs((mining_time * 60.) as u64)),
            block_time: Some(Duration::from_secs(
                (response.block_time * 60.) as u64,
            )),
        };
    }
}

#[cfg(test)]
mod tests {
    use crate::gas_oracle::{ETHGasStationOracle, GasOracle};
    use crate::transaction::Priority;

    #[tokio::test]
    async fn test_eth_gas_station_oracle() {
        // setup
        tracing_subscriber::fmt::init();
        let gas_oracle = ETHGasStationOracle::new(
            "c5396ceb50a0c347dba8de605f47ffc8e9fd347495b57da6a2d537f78848",
        );
        let invalid_gas_oracle1 = ETHGasStationOracle::new("invalid");
        let invalid_gas_oracle2 = ETHGasStationOracle::new("");

        // ok => priority low
        let result = gas_oracle.gas_info(Priority::Low).await;
        assert!(result.is_ok());

        // ok => priority normal
        let result = gas_oracle.gas_info(Priority::Normal).await;
        assert!(result.is_ok());

        // ok => priority high
        let result = gas_oracle.gas_info(Priority::High).await;
        assert!(result.is_ok());

        // ok => priority ASAP
        let result = gas_oracle.gas_info(Priority::ASAP).await;
        assert!(result.is_ok());

        // ok => invalid API key works (for some reason)
        let result = invalid_gas_oracle1.gas_info(Priority::Normal).await;
        assert!(result.is_ok());

        // ok => empty API key works (for some reason)
        let result = invalid_gas_oracle2.gas_info(Priority::Normal).await;
        assert!(result.is_ok());
    }
}
