use async_trait::async_trait;
use ethers::providers::{FromErr, Middleware, PendingTransaction};
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::{
    BlockId, NameOrAddress, TransactionReceipt, TxHash, U256, U64,
};
use std::marker::PhantomData;

// Middleware mock.

#[derive(Debug)]
pub struct Provider<M: Middleware> {
    inner: PhantomData<M>,
}

impl<M: Middleware> Provider<M> {
    pub fn new() -> Self {
        return Self { inner: PhantomData };
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderOutput<M: Middleware> {
    #[error("provider mock output: inner error -- {0}")]
    InnerError(M::Error),
    // TODO
}

impl<M: Middleware> FromErr<M::Error> for ProviderOutput<M> {
    fn from(src: M::Error) -> ProviderOutput<M> {
        ProviderOutput::InnerError(src)
    }
}

#[async_trait]
impl<M: Middleware> Middleware for Provider<M> {
    type Error = ProviderOutput<M>;
    type Provider = M::Provider;
    type Inner = M;

    fn inner(&self) -> &M {
        unreachable!()
    }

    async fn estimate_gas(
        &self,
        tx: &TypedTransaction,
    ) -> Result<U256, Self::Error> {
        todo!();
    }

    async fn send_transaction<T: Into<TypedTransaction> + Send + Sync>(
        &self,
        tx: T,
        block: Option<BlockId>,
    ) -> Result<PendingTransaction<'_, M::Provider>, Self::Error> {
        todo!();
    }

    async fn get_block_number(&self) -> Result<U64, Self::Error> {
        todo!();
    }

    async fn get_transaction_receipt<T: Send + Sync + Into<TxHash>>(
        &self,
        transaction_hash: T,
    ) -> Result<Option<TransactionReceipt>, Self::Error> {
        todo!();
    }

    async fn get_transaction_count<T: Into<NameOrAddress> + Send + Sync>(
        &self,
        from: T,
        block: Option<BlockId>,
    ) -> Result<U256, Self::Error> {
        todo!();
    }
}
