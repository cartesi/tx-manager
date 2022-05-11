use ethers::types::transaction::eip2930::AccessList;
use ethers::types::{
    Address, Eip1559TransactionRequest, NameOrAddress, H256, U256, U64,
};
use ethers::utils;
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
pub enum Priority {
    Low,
    Normal,
    High,
    ASAP,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub priority: Priority,
    pub from: Address,
    pub to: Address,
    pub value: Value,
    pub confirmations: u32,
    // pub call_data: Option<Bytes>, // smart contract payload
}

impl Transaction {
    pub fn to_eip_1559_transaction_request(
        &self,
        nonce: U256,
        max_priority_fee_per_gas: U256,
        max_fee_per_gas: U256,
    ) -> Eip1559TransactionRequest {
        Eip1559TransactionRequest {
            from: Some(self.from),
            to: Some(NameOrAddress::Address(self.to)),
            gas: None, // must be set after
            value: Some(self.value.into()),
            data: None,
            nonce: Some(nonce),
            access_list: AccessList::default(),
            max_priority_fee_per_gas: Some(max_priority_fee_per_gas),
            max_fee_per_gas: Some(max_fee_per_gas),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Value {
    Number(U256),
    // All,
    // Nothing,
}

impl From<Value> for U256 {
    fn from(value: Value) -> Self {
        match value {
            Value::Number(v) => v,
        }
    }
}
