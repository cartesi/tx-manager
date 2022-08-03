use ethers::core::types::Bytes;
use ethers::types::transaction::eip2930::AccessList;
use ethers::types::{Address, Eip1559TransactionRequest, NameOrAddress, H256, U256, U64};
use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Normal,
    High,
    ASAP,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub from: Address,
    pub to: Address,
    pub value: Value,
    pub call_data: Option<Bytes>, // smart contract payload
}

impl Transaction {
    pub fn to_eip_1559_transaction_request(
        &self,
        chain_id: U64,
        nonce: U256,
        max_priority_fee_per_gas: U256,
        max_fee_per_gas: U256,
    ) -> Eip1559TransactionRequest {
        Eip1559TransactionRequest {
            chain_id: Some(chain_id),
            from: Some(self.from),
            to: Some(NameOrAddress::Address(self.to)),
            gas: None, // must be set after
            value: self.value.into(),
            data: self.call_data.clone(),
            nonce: Some(nonce),
            access_list: AccessList::default(),
            max_priority_fee_per_gas: Some(max_priority_fee_per_gas),
            max_fee_per_gas: Some(max_fee_per_gas),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Value {
    Number(U256), // in wei
    Nothing,
    // All,
}

impl From<Value> for Option<U256> {
    fn from(value: Value) -> Self {
        match value {
            Value::Number(v) => Some(v),
            Value::Nothing => Some(0.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StaticTxData {
    /// Nonce of the current transaction.
    pub nonce: U256,

    /// Information about the transaction being currently processed.
    pub transaction: Transaction,

    pub confirmations: usize,
    pub priority: Priority,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubmittedTxs {
    /// Hashes of the pending transactions sent to the transaction pool.
    pub txs_hashes: Vec<H256>,
}

impl<'a> IntoIterator for &'a SubmittedTxs {
    type Item = &'a H256;
    type IntoIter = std::slice::Iter<'a, H256>;

    fn into_iter(self) -> Self::IntoIter {
        self.txs_hashes.iter()
    }
}

impl SubmittedTxs {
    pub fn new() -> Self {
        Self {
            txs_hashes: Vec::new(),
        }
    }

    pub fn add_tx_hash(&mut self, tx_hash: H256) {
        self.txs_hashes.push(tx_hash);
    }

    pub fn tx_count(&self) {
        self.txs_hashes.len();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistentState {
    /// Information about the transaction being currently processed.
    pub tx_data: StaticTxData,

    /// Hashes of the pending transactions sent to the transaction pool.
    pub submitted_txs: SubmittedTxs,
}
