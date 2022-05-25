use anyhow::{anyhow, Result};
use async_trait::async_trait;

use tx_manager::gas_oracle::GasInfo;
use tx_manager::transaction::Priority;

#[derive(Debug, thiserror::Error)]
pub enum GasOracleError {
    #[error("gas oracle mock error: gas info")]
    GasInfo,
}

#[derive(Debug)]
pub struct GasOracle {
    pub gas_info_output: Option<GasInfo>,
}

impl GasOracle {
    pub fn new() -> Self {
        Global::setup();
        Self {
            gas_info_output: None,
        }
    }

    pub fn global() -> &'static Global {
        unsafe { &GLOBAL }
    }
}

#[async_trait]
impl tx_manager::gas_oracle::GasOracle for GasOracle {
    async fn gas_info(&self, _: Priority) -> Result<GasInfo> {
        unsafe { GLOBAL.gas_info_n += 1 };
        self.gas_info_output
            .ok_or(anyhow!(GasOracleError::GasInfo))
            .map(|x| x.clone())
    }
}

pub struct Global {
    pub gas_info_n: i32,
}

static mut GLOBAL: Global = Global::default();

impl Global {
    const fn default() -> Global {
        Global { gas_info_n: 0 }
    }

    fn setup() {
        unsafe {
            GLOBAL = Global::default();
        }
    }
}
