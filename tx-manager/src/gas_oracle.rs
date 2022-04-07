use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::transaction::Priority;

use tracing::error;

#[derive(Debug, Clone, Copy)]
pub enum Error {
    HTTPGet,
    StatusNotOk,
    ResponseToJSON,
}

#[derive(Debug)]
pub struct GasOracle {
    pub api_key: String,
}

#[derive(Debug)]
pub struct GasInfo {
    pub gas_price: i32,  // 10 * gwei
    pub wait_time: i32,  // seconds
    pub block_time: i32, // seconds
}

fn log_err<E: std::fmt::Debug>(lib_err: Error, inner_err: E) -> Error {
    error!("{:?} {:#?}", lib_err, inner_err);
    return lib_err;
}

impl GasOracle {
    #[tracing::instrument]
    pub async fn estimate_eip1559_fees(
        &self,
        priority: Priority,
    ) -> Result<GasInfo, Error> {
        let url = format!(
            "https://ethgasstation.info/api/ethgasAPI.json?api-key={}",
            self.api_key
        );

        let response = reqwest::get(url)
            .await
            .map_err(|err| log_err(Error::HTTPGet, err))?;

        if response.status() != StatusCode::OK {
            return Err(log_err(
                Error::StatusNotOk,
                format!("status code: {:?}", response.status()),
            ));
        }

        let response = response
            .json::<ETHGasStationResponse>()
            .await
            .map_err(|err| log_err(Error::ResponseToJSON, err))?;

        // TODO: create response_to_gasinfo function
        let block_time = response.block_time;
        let (gas_price, wait_time) = match priority {
            Priority::Low => (response.low, response.low_time),
            Priority::Normal => (response.average, response.average_time),
            Priority::High => (response.fastest, response.fastest_time),
        };

        return Ok(GasInfo {
            gas_price,
            wait_time: (wait_time * 60.) as i32,
            block_time: (block_time * 60.) as i32,
        });
    }
}

#[derive(Debug, Serialize, Deserialize)]
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
