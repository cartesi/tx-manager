use async_trait::async_trait;
use ethers::providers::{FromErr, Middleware, PendingTransaction};
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::{
    BlockId, Bloom, NameOrAddress, TransactionReceipt, TxHash, H256, U256, U64,
};
use std::marker::PhantomData;
use std::str::FromStr;

// Global state used to simulate requests to the blockchain.

static mut NONCE: u32 = 1;
static mut BLOCK_NUMBER: u32 = 10;
static mut TRANSACTION_HASH: H256 =
    h256("0x824384376c5972498c6fcafe71fd8cad1689f64e7d5e270d025a898638c0c34d");

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
        Ok(u256(21000))
    }

    async fn send_transaction<T: Into<TypedTransaction> + Send + Sync>(
        &self,
        _: T,
        _: Option<BlockId>,
    ) -> Result<PendingTransaction<'_, M::Provider>, Self::Error> {
        unsafe {
            Ok(PendingTransaction::new(TRANSACTION_HASH, self.provider()))
        }
    }

    async fn get_block_number(&self) -> Result<U64, Self::Error> {
        unsafe {
            let block = u64(BLOCK_NUMBER);
            BLOCK_NUMBER += 1;
            Ok(block)
        }
    }

    async fn get_transaction_receipt<T: Send + Sync + Into<TxHash>>(
        &self,
        _: T,
    ) -> Result<Option<TransactionReceipt>, Self::Error> {
        let transaction_hash = "0x824384376c5972498c6fcafe71fd8cad1689f64e7d5e270d025a898638c0c34d";
        let block_hash = "0x55ae43d3511e327dc532855510d110676d340aa1bbba369b4b98896d86559586";

        let receipt = TransactionReceipt {
            transaction_hash: h256(transaction_hash),
            transaction_index: u64(13),
            block_hash: Some(h256(block_hash)),
            block_number: Some(u64(10736418)),
            cumulative_gas_used: u256(2000000),
            gas_used: Some(u256(30000)),
            contract_address: None,
            logs: vec![],
            status: Some(u64(1)),
            root: None, // TODO
            logs_bloom: Bloom::zero(),
            transaction_type: Some(u64(1)),
            effective_gas_price: Some(u256(1000000000)),
        };
        Ok(Some(receipt))
    }

    async fn get_transaction_count<T: Into<NameOrAddress> + Send + Sync>(
        &self,
        _: T,
        _: Option<BlockId>,
    ) -> Result<U256, Self::Error> {
        unsafe { Ok(u256(NONCE)) }
    }
}

fn u64(n: u32) -> U64 {
    U64::from_dec_str(&n.to_string()).unwrap()
}

fn u256(n: u32) -> U256 {
    U256::from_dec_str(&n.to_string()).unwrap()
}

fn h256(s: &str) -> H256 {
    H256::from_str(s).unwrap()
}
