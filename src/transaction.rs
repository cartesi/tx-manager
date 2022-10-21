use ethers::core::types::Bytes;
use ethers::types::transaction::eip2930::AccessList;
use ethers::types::{
    transaction::eip2718::TypedTransaction, Address, Eip1559TransactionRequest, NameOrAddress,
    TransactionRequest, H256, U256,
};
use serde::{Deserialize, Serialize};

use crate::gas_oracle::GasInfo;
use crate::Chain;

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

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Value {
    Number(U256), // in wei for ethereum
    Nothing,
    // All,
}

impl From<Value> for U256 {
    fn from(value: Value) -> Self {
        match value {
            Value::Number(v) => v,
            Value::Nothing => 0.into(),
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

impl StaticTxData {
    pub fn to_typed_transaction(&self, chain: &Chain, gas_info: GasInfo) -> TypedTransaction {
        let from = Some(self.transaction.from);
        let to = Some(NameOrAddress::Address(self.transaction.to));
        let value = Some(self.transaction.value.into());
        let data = self.transaction.call_data.clone();
        let nonce = Some(self.nonce);
        let chain_id = Some(chain.id.into());

        match gas_info {
            GasInfo::Legacy(legacy_gas_info) => {
                TypedTransaction::Legacy(TransactionRequest {
                    from,
                    to,
                    gas: None, // must be set after
                    gas_price: Some(legacy_gas_info.gas_price),
                    value,
                    data,
                    nonce,
                    chain_id,
                })
            }
            GasInfo::EIP1559(eip1559_gas_info) => {
                TypedTransaction::Eip1559(Eip1559TransactionRequest {
                    from,
                    to,
                    gas: None, // must be set after
                    value,
                    data,
                    nonce,
                    access_list: AccessList::default(),
                    // max_priority_fee must be set (guaranteed by get_gas_oracle_info)
                    max_priority_fee_per_gas: Some(eip1559_gas_info.max_priority_fee.unwrap()),
                    max_fee_per_gas: Some(eip1559_gas_info.max_fee),
                    chain_id,
                })
            }
        }
    }
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

    pub fn contains(&mut self, hash: H256) -> bool {
        self.txs_hashes.contains(&hash)
    }

    pub fn add(&mut self, hash: H256) {
        self.txs_hashes.push(hash);
    }

    pub fn len(&self) -> usize {
        self.txs_hashes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.txs_hashes.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistentState {
    /// Information about the transaction being currently processed.
    pub tx_data: StaticTxData,

    /// Hashes of the pending transactions sent to the transaction pool.
    pub submitted_txs: SubmittedTxs,
}
