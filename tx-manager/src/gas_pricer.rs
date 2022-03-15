// c5396ceb50a0c347dba8de605f47ffc8e9fd347495b57da6a2d537f78848
// let url = "https://ethgasstation.info/api/ethgasAPI.json?api-key={}";

use ethers::types::U256;

use crate::transaction::Priority;

pub struct GasPricer {}

impl GasPricer {
    pub fn estimate_eip1559_fees(&self, priority: Priority) -> (U256, U256) {
        return ((5 as u32).into(), (5 as u32).into());
    }
}
