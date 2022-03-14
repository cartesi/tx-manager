use ethers::types::{Address, U256};

#[derive(Debug)]
pub struct Transaction {
    pub label: &'static str, // TODO
    pub priority: Priority,
    pub from: Address,
    pub to: Address,
    pub value: Value,
    // pub call_data: Option<Bytes>, // smart contract payload
}

impl PartialEq for Transaction {
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label
    }
}

impl Eq for Transaction {}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    Normal,
    High,
}

#[derive(Debug)]
pub enum Value {
    Value(U256),
    // All,
    // Nothing,
}

impl Value {
    pub fn get(&self) -> U256 {
        match self {
            Self::Value(v) => *v,
        }
    }
}
