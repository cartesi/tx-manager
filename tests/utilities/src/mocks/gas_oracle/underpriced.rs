use async_trait::async_trait;
use ethers::types::U256;
use tx_manager::{
    gas_oracle::{EIP1559GasInfo, GasInfo, GasOracle, GasOracleInfo},
    transaction::Priority,
};

/// Guarantees that from the second transaction onward the max fee will be
/// underpriced.
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

    async fn get_info(&self, _: Priority) -> Result<GasOracleInfo, Self::Error> {
        // The first transaction has a max_fee of 2 gwei.
        // Other transactions have a max_fee of 1 gwei.
        let max_fee = U256::from(if unsafe { GLOBAL.flag } { 2e9 } else { 1e9 } as u32);
        let result = Ok(GasOracleInfo {
            gas_info: GasInfo::EIP1559(EIP1559GasInfo {
                max_fee,
                max_priority_fee: None,
            }),
            mining_time: None,
            block_time: None,
        });
        unsafe { GLOBAL.flag = false };
        result
    }
}

pub struct Global {
    pub flag: bool,
}

static mut GLOBAL: Global = Global::default();

impl Global {
    const fn default() -> Global {
        Global { flag: true }
    }

    fn setup() {
        unsafe {
            GLOBAL = Global::default();
        }
    }
}
