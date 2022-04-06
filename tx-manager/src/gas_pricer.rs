use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::transaction::Priority;

pub enum GasPricerError {
    TODO,
}

pub struct GasPricer {
    api_key: String,
    // c5396ceb50a0c347dba8de605f47ffc8e9fd347495b57da6a2d537f78848
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GasPrices {
    block_time: f32,
    fastest: i32,
    fast: i32,
    average: i32,
    safe_low: i32,
    fastest_wait: f32,
    fast_wait: f32,
    avg_wait: f32,
    safe_low_wait: f32,
}

// max_fee_per_gas, max_priority_fee_per_gas
pub struct GasInfo {
    gas_price: f32,  // gwei
    wait_time: i32,  // seconds
    block_time: i32, // seconds
}

impl GasPricer {
    pub async fn estimate_eip1559_fees(
        &self,
        priority: Priority,
    ) -> Result<GasInfo, GasPricerError> {
        let url = "https://ethgasstation.info/api/ethgasAPI.json?api-key={}";

        let response =
            reqwest::get(url).await.map_err(|_| GasPricerError::TODO)?;

        if response.status() != StatusCode::OK {
            return Err(GasPricerError::TODO);
        }

        let gas_prices = response
            .json::<GasPrices>()
            .await
            .map_err(|_| GasPricerError::TODO)?;

        let block_time = gas_prices.block_time;
        let (gas_price, wait_time) = match priority {
            Low => (gas_prices.safe_low, gas_prices.safe_low_wait),
            Medium => (gas_prices.average, gas_prices.avg_wait),
            High => (gas_prices.fastest, gas_prices.fastest_wait),
        };

        return Ok(GasInfo {
            gas_price: (gas_price as f32) / 10.,
            wait_time: (wait_time * 60.) as i32,
            block_time: (block_time * 60.) as i32,
        });
    }
}
