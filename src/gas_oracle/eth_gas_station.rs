use async_trait::async_trait;
use core::time::Duration;
use ethers::types::U256;
use reqwest::StatusCode;
use serde::Deserialize;
use std::fmt::Debug;
use tracing::trace;

use crate::gas_oracle::{EIP1559GasInfo, GasInfo, GasOracle, GasOracleInfo};
use crate::transaction::Priority;

/// Implementation that uses the ETH Gas Station API.

#[derive(Debug, thiserror::Error)]
pub enum ETHGasStationError {
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
    type Error = ETHGasStationError;

    #[tracing::instrument(level = "trace")]
    async fn get_info(&self, priority: Priority) -> Result<GasOracleInfo, Self::Error> {
        let url = format!(
            "https://ethgasstation.info/api/ethgasAPI.json?api-key={}",
            self.api_key
        );

        let res = reqwest::get(url)
            .await
            .map_err(ETHGasStationError::Request)?;
        if res.status() != StatusCode::OK {
            return Err(ETHGasStationError::StatusCode(res.status()));
        }

        let bytes = &res.bytes().await.map_err(ETHGasStationError::Request)?;
        let response = serde_json::from_slice(bytes).map_err(ETHGasStationError::ParseResponse)?;
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

impl From<(ETHGasStationResponse, Priority)> for GasOracleInfo {
    fn from((response, priority): (ETHGasStationResponse, Priority)) -> Self {
        let (gas_price, mining_time) = match priority {
            Priority::Low => (response.low, response.low_time),
            Priority::Normal => (response.average, response.average_time),
            Priority::High => (response.fast, response.fast_time),
            Priority::ASAP => (response.fastest, response.fastest_time),
        };

        // max fee from 10*gwei to wei
        let max_fee = U256::from(gas_price).checked_mul(U256::exp10(10)).unwrap();
        let max_priority_fee = None;
        let mining_time = Some(Duration::from_secs((mining_time * 60.) as u64));
        let block_time = Some(Duration::from_secs((response.block_time) as u64));

        GasOracleInfo {
            gas_info: GasInfo::EIP1559(EIP1559GasInfo {
                max_fee,
                max_priority_fee,
            }),
            mining_time,
            block_time,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::gas_oracle::{EIP1559GasInfo, ETHGasStationOracle, GasOracle, GasOracleInfo};
    use crate::transaction::Priority;

    use super::ETHGasStationError;

    // Auxiliary.
    fn unwrap_eip1559_gas_info(
        result: Result<GasOracleInfo, ETHGasStationError>,
    ) -> EIP1559GasInfo {
        assert!(result.is_ok(), "{:?}", result);
        let eip1559_gas_info: Result<EIP1559GasInfo, &'static str> =
            result.unwrap().gas_info.try_into();
        assert!(eip1559_gas_info.is_ok(), "{:?}", eip1559_gas_info);
        eip1559_gas_info.unwrap()
    }

    #[tokio::test]
    async fn test_eth_gas_station_oracle_ok() {
        // setup
        // tracing_subscriber::fmt::init();
        let gas_oracle = ETHGasStationOracle::new("works".to_string());

        // ok => priority low
        let result = gas_oracle.get_info(Priority::Low).await;
        assert!(result.is_ok(), "{:?}", result);
        let eip1559_gas_info_low = unwrap_eip1559_gas_info(result);
        assert!(eip1559_gas_info_low.max_priority_fee.is_none());

        // ok => priority normal
        let result = gas_oracle.get_info(Priority::Normal).await;
        assert!(result.is_ok());
        let eip1559_gas_info_normal = unwrap_eip1559_gas_info(result);
        assert!(eip1559_gas_info_normal.max_priority_fee.is_none());

        // ok => priority high
        let result = gas_oracle.get_info(Priority::High).await;
        assert!(result.is_ok());
        let eip1559_gas_info_high = unwrap_eip1559_gas_info(result);
        assert!(eip1559_gas_info_high.max_priority_fee.is_none());

        // ok => priority ASAP
        let result = gas_oracle.get_info(Priority::ASAP).await;
        assert!(result.is_ok());
        let eip1559_gas_info_asap = unwrap_eip1559_gas_info(result);
        assert!(eip1559_gas_info_asap.max_priority_fee.is_none());

        assert!(eip1559_gas_info_low.max_fee <= eip1559_gas_info_normal.max_fee);
        assert!(eip1559_gas_info_normal.max_fee <= eip1559_gas_info_high.max_fee);
        assert!(eip1559_gas_info_high.max_fee <= eip1559_gas_info_asap.max_fee);
    }

    #[tokio::test]
    async fn test_eth_gas_station_oracle_invalid_api_key() {
        // setup
        let invalid1 = ETHGasStationOracle::new("invalid".to_string());
        let invalid2 = ETHGasStationOracle::new("".to_string());

        // ok => invalid API key works (for some reason)
        let result = invalid1.get_info(Priority::Normal).await;
        assert!(result.is_ok());

        // ok => empty API key works (for some reason)
        let result = invalid2.get_info(Priority::Normal).await;
        assert!(result.is_ok());
    }
}
