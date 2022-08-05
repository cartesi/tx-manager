use async_trait::async_trait;
use ethers::prelude::U256;
use tx_manager::{
    gas_oracle::{GasInfo, GasOracle},
    transaction::Priority,
};

#[derive(Debug)]
pub struct UnderpricedGasOracle {}

impl UnderpricedGasOracle {
    pub fn new() -> Self {
        Global::setup();
        Self {}
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UnderpricedGasOracleError {}

#[async_trait]
impl GasOracle for UnderpricedGasOracle {
    type Error = UnderpricedGasOracleError;

    async fn gas_info(&self, _: Priority) -> Result<GasInfo, Self::Error> {
        let result = Ok(GasInfo {
            max_fee: U256::from(2_000_000_000 / unsafe { GLOBAL.n }),
            max_priority_fee: None,
            mining_time: None,
            block_time: None,
        });
        unsafe { GLOBAL.n += 1 };
        result
    }
}

pub struct Global {
    pub n: u32,
}

static mut GLOBAL: Global = Global::default();

impl Global {
    const fn default() -> Global {
        Global { n: 1 }
    }

    fn setup() {
        unsafe {
            GLOBAL = Global::default();
        }
    }
}
