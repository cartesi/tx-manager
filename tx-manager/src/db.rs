use ethers::types::TransactionReceipt;

use crate::{manager::SendError, transaction::Transaction};

#[derive(Debug)]
pub struct Database {}

impl Database {
    pub fn get_transaction_receipt_for(
        &self,
        transaction: &Transaction,
    ) -> Result<Option<TransactionReceipt>, SendError> {
        Ok(None) // TODO
    }
}
