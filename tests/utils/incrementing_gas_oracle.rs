use async_trait::async_trait;
use ethers::prelude::U256;
use tx_manager::{
    gas_oracle::{EIP1559GasInfo, GasInfo, GasOracle, GasOracleInfo},
    transaction::Priority,
};

#[derive(Debug)]
pub struct IncrementingGasOracle {}

impl IncrementingGasOracle {
    pub fn new() -> Self {
        Global::setup();
        Self {}
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IncrementingGasOracleError {}

#[async_trait]
impl GasOracle for IncrementingGasOracle {
    type Error = IncrementingGasOracleError;

    async fn gas_oracle_info(&self, _: Priority) -> Result<GasOracleInfo, Self::Error> {
        let result = Ok(GasOracleInfo {
            gas_info: GasInfo::EIP1559(EIP1559GasInfo {
                max_fee: U256::from(2_000_000_000 + unsafe { GLOBAL.n }),
                max_priority_fee: Some(U256::from(100_000)),
            }),
            mining_time: None,
            block_time: None,
        });
        unsafe { GLOBAL.n += GLOBAL.n };
        result
    }
}

pub struct Global {
    pub n: u32,
}

static mut GLOBAL: Global = Global::default();

impl Global {
    const fn default() -> Global {
        Global { n: 100 }
    }

    fn setup() {
        unsafe {
            GLOBAL = Global::default();
        }
    }
}
