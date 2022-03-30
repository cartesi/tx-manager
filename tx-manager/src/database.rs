use async_trait::async_trait;

use ethers::types::Eip1559TransactionRequest;

use crate::transaction::Transaction;

#[async_trait]
pub trait Database {
    type Error;

    async fn store_transaction(
        &self,
        transaction: &Transaction,
    ) -> Result<(), Self::Error>;

    async fn update_transaction(
        &self,
        transaction_request: Eip1559TransactionRequest,
    ) -> Result<(), Self::Error>;

    async fn clear_transaction(&self) -> Result<(), Self::Error>;
}
