use async_trait::async_trait;

use tx_manager::gas_oracle::GasOracleInfo;
use tx_manager::transaction::Priority;

#[derive(Debug)]
pub struct GasOracle {
    pub gas_oracle_info_output: Option<GasOracleInfo>,
}

impl GasOracle {
    pub fn new() -> Self {
        Global::setup();
        Self {
            gas_oracle_info_output: None,
        }
    }

    pub fn global() -> &'static Global {
        unsafe { &GLOBAL }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GasOracleError {
    #[error("gas oracle mock error: gas info")]
    GasInfo,
}

#[async_trait]
impl tx_manager::gas_oracle::GasOracle for GasOracle {
    type Error = GasOracleError;

    async fn get_info(&self, _: Priority) -> Result<GasOracleInfo, Self::Error> {
        unsafe { GLOBAL.gas_info_n += 1 };
        self.gas_oracle_info_output.ok_or(GasOracleError::GasInfo)
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
