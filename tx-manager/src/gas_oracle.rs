use async_trait::async_trait;
use reqwest::StatusCode;
use serde::Deserialize;
use thiserror::Error;
use tracing::{debug, error};

use crate::transaction::Priority;

#[async_trait]
pub trait GasOracle {
    async fn gas_info(&self, priority: Priority) -> Result<GasInfo, Error>;
}

#[derive(Debug)]
pub struct GasInfo {
    pub gas_price: i32,  // 10 * gwei
    pub wait_time: i32,  // seconds
    pub block_time: i32, // seconds
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("request error: {0}")]
    Request(#[from] reqwest::Error),

    #[error("invalid status code {0}")]
    StatusNotOk(reqwest::StatusCode),

    #[error("could not parse the response to JSON: {0}")]
    Decode(#[from] serde_json::error::Error),
}

// Implementation using the ETH Gas Station API.

#[derive(Debug)]
pub struct ETHGasStationOracle {
    pub api_key: String,
}

#[async_trait]
impl GasOracle for ETHGasStationOracle {
    #[tracing::instrument]
    async fn gas_info(&self, priority: Priority) -> Result<GasInfo, Error> {
        let url = format!(
            "https://ethgasstation.info/api/ethgasAPI.json?api-key={}",
            self.api_key
        );

        let res = reqwest::get(url).await?;
        if res.status() != StatusCode::OK {
            return Err(Error::StatusNotOk(res.status()));
        }

        let response = serde_json::from_slice(&res.bytes().await?)?;
        let gas_info = (response, priority).into();
        debug!("gas info: {:?}", gas_info);
        return Ok(gas_info);
    }
}

#[derive(Debug, Deserialize)]
struct ETHGasStationResponse {
    block_time: f32,
    fastest: i32,
    fast: i32,
    average: i32,
    #[serde(rename = "safeLow")]
    low: i32,
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
        let (gas_price, wait_time) = match priority {
            Priority::Low => (response.low, response.low_time),
            Priority::Normal => (response.average, response.average_time),
            Priority::High => (response.fast, response.fast_time),
            Priority::ASAP => (response.fastest, response.fastest_time),
        };

        return GasInfo {
            gas_price,
            wait_time: (wait_time * 60.) as i32,
            block_time: (response.block_time * 60.) as i32,
        };
    }
}
