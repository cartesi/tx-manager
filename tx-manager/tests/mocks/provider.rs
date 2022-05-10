use async_trait::async_trait;
use ethers::providers::{FromErr, Middleware, PendingTransaction};
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::{
    BlockId, NameOrAddress, TransactionReceipt, TxHash, U256, U64,
};
use std::marker::PhantomData;
use std::ptr;

// Middleware mock.

#[derive(Debug)]
pub struct Provider<M: Middleware> {
    inner: PhantomData<M>,
    block_number: u32,
}

impl<M: Middleware> Provider<M> {
    pub fn new() -> Self {
        return Self {
            inner: PhantomData,
            block_number: 10,
        };
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError<M: Middleware> {
    #[error("provider mock error: inner error -- {0}")]
    InnerError(M::Error),
    // TODO
}

impl<M: Middleware> FromErr<M::Error> for ProviderError<M> {
    fn from(src: M::Error) -> ProviderError<M> {
        ProviderError::InnerError(src)
    }
}

#[async_trait]
impl<M: Middleware> Middleware for Provider<M> {
    type Error = ProviderError<M>;
    type Provider = M::Provider;
    type Inner = M;

    fn inner(&self) -> &M {
        unreachable!()
    }

    async fn estimate_gas(
        &self,
        _: &TypedTransaction,
    ) -> Result<U256, Self::Error> {
        todo!();
    }

    async fn send_transaction<T: Into<TypedTransaction> + Send + Sync>(
        &self,
        _: T,
        _: Option<BlockId>,
    ) -> Result<PendingTransaction<'_, M::Provider>, Self::Error> {
        todo!();
    }

    async fn get_block_number(&self) -> Result<U64, Self::Error> {
        let self_ptr = ptr::addr_of!(self) as *mut Self;
        let block = U64::from_dec_str(&self.block_number.to_string()).unwrap();
        unsafe {
            // TODO: I think this is undefined behavior
            (*self_ptr).block_number = self.block_number + 1;
        }
        Ok(block)
    }

    async fn get_transaction_receipt<T: Send + Sync + Into<TxHash>>(
        &self,
        _: T,
    ) -> Result<Option<TransactionReceipt>, Self::Error> {
        let receipt_str = r#"{
            "transactionHash": "0x824384376c5972498c6fcafe71fd8cad1689f64e7d5e270d025a898638c0c34d",
            "transactionIndex": "0xd",
            "blockHash": "0x55ae43d3511e327dc532855510d110676d340aa1bbba369b4b98896d86559586",
            "blockNumber": "0xa3d322",
            "cumulativeGasUsed": "0x207a5b",
            "gasUsed": "0x6a40",
            "contractAddress": null,
            "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
            "logs": [],
            "type": "0x2",
            "effectiveGasPrice": "0x3b9aca07"
        }"#;
        let mut receipt: TransactionReceipt =
            serde_json::from_str(receipt_str).unwrap();
        receipt.block_number = U64::from_dec_str("10").ok();
        Ok(Some(receipt))
    }

    async fn get_transaction_count<T: Into<NameOrAddress> + Send + Sync>(
        &self,
        _: T,
        _: Option<BlockId>,
    ) -> Result<U256, Self::Error> {
        todo!();
    }
}
