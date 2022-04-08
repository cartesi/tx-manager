use ethers::types::{Address, U256};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub priority: Priority,
    pub from: Address,
    pub to: Address,
    pub value: Value,
    pub confirmations: usize,
    // pub call_data: Option<Bytes>, // smart contract payload
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
pub enum Priority {
    Low,
    Normal,
    High,
    ASAP,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Value {
    Number(U256),
    // All,
    // Nothing,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ValueError {
    CouldNotConvertValue,
}

impl TryFrom<Value> for U256 {
    type Error = ValueError;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Number(v) => Ok(v),
        }
    }
}
