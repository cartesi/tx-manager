use async_trait::async_trait;
use ethers::providers::{
    FromErr, Middleware, MockProvider, PendingTransaction, Provider,
};
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::{
    Address, Block, BlockId, Bloom, NameOrAddress, Signature,
    TransactionReceipt, TxHash, H256, U256, U64,
};
use std::str::FromStr;

// Global state used to simulate the blockchain.

pub struct MockMiddlewareState {
    pub nonce: u32,
    pub block_number: u32,
    pub sent_transactions: Vec<TxHash>,
}

static mut STATE: MockMiddlewareState = setup_state();

const fn setup_state() -> MockMiddlewareState {
    MockMiddlewareState {
        nonce: 1,
        block_number: 10,
        sent_transactions: Vec::new(),
    }
}

// Middleware mock.

#[derive(Debug)]
pub struct MockMiddleware {
    provider: (Provider<MockProvider>, MockProvider),
    pub estimate_gas: Option<U256>,
    pub get_block: Option<()>,
    pub get_block_number: Option<()>,
    pub get_transaction_count: Option<()>,
    pub get_transaction_receipt: Option<()>,
    pub send_transaction: Option<TxHash>,
    pub sign_transaction: Option<()>,
}

impl MockMiddleware {
    pub fn new() -> Self {
        unsafe {
            STATE = setup_state();
        }
        Self {
            provider: Provider::mocked(),
            estimate_gas: None,
            get_block: None,
            get_block_number: None,
            get_transaction_count: None,
            get_transaction_receipt: None,
            send_transaction: None,
            sign_transaction: None,
        }
    }

    pub fn setup_state() {
        unsafe {
            STATE = setup_state();
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MockMiddlewareError {
    #[error("mock middleware error: estimate gas")]
    EstimateGas,

    #[error("mock middleware error: get block")]
    GetBlock,

    #[error("mock middleware error: get block number")]
    GetBlockNumber,

    #[error("mock middleware error: get transaction count")]
    GetTransactionCount,

    #[error("mock middleware error: get transaction receipt")]
    GetTransactionReceipt,

    #[error("mock middleware error: send transaction")]
    SendTransaction,

    #[error("mock middleware error: sign transaction")]
    SignTransaction,
}

impl FromErr<MockMiddlewareError> for MockMiddlewareError {
    fn from(err: MockMiddlewareError) -> MockMiddlewareError {
        err
    }
}

#[async_trait]
impl Middleware for MockMiddleware {
    type Error = MockMiddlewareError;
    type Provider = MockProvider;
    type Inner = Self;

    fn inner(&self) -> &Self::Inner {
        unreachable!()
    }

    fn provider(&self) -> &Provider<Self::Provider> {
        &self.provider.0
    }

    async fn estimate_gas(
        &self,
        _: &TypedTransaction,
    ) -> Result<U256, Self::Error> {
        self.estimate_gas.ok_or(MockMiddlewareError::EstimateGas)
    }

    async fn get_block<T: Into<BlockId> + Send + Sync>(
        &self,
        _: T,
    ) -> Result<Option<Block<TxHash>>, Self::Error> {
        let mut block = Block::<TxHash>::default();
        block.base_fee_per_gas = Some(u256(250));
        match self.get_block {
            None => Err(MockMiddlewareError::GetBlock),
            Some(_) => Ok(Some(block)),
        }
    }

    async fn get_block_number(&self) -> Result<U64, Self::Error> {
        self.get_block_number
            .ok_or(MockMiddlewareError::GetBlockNumber)?;
        unsafe {
            let block = u64(STATE.block_number);
            STATE.block_number += 1;
            Ok(block)
        }
    }

    async fn get_transaction_count<T: Into<NameOrAddress> + Send + Sync>(
        &self,
        _: T,
        _: Option<BlockId>,
    ) -> Result<U256, Self::Error> {
        self.get_transaction_count
            .ok_or(MockMiddlewareError::GetTransactionCount)?;
        unsafe { Ok(u256(STATE.nonce)) }
    }

    async fn get_transaction_receipt<T: Send + Sync + Into<TxHash>>(
        &self,
        _: T,
    ) -> Result<Option<TransactionReceipt>, Self::Error> {
        self.get_transaction_receipt
            .ok_or(MockMiddlewareError::GetTransactionReceipt)?;

        let transaction_hash = "0x824384376c5972498c6fcafe71fd8cad1689f64e7d5e270d025a898638c0c34d";
        let block_hash = "0x55ae43d3511e327dc532855510d110676d340aa1bbba369b4b98896d86559586";

        let receipt = TransactionReceipt {
            transaction_hash: h256(transaction_hash),
            transaction_index: u64(13),
            block_hash: Some(h256(block_hash)),
            block_number: Some(u64(9)),
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

    async fn send_transaction<T: Into<TypedTransaction> + Send + Sync>(
        &self,
        _: T,
        _: Option<BlockId>,
    ) -> Result<PendingTransaction<'_, Self::Provider>, Self::Error> {
        let hash = self
            .send_transaction
            .ok_or(MockMiddlewareError::SendTransaction)?;
        let pending_transaction =
            PendingTransaction::new(hash, self.provider());
        unsafe {
            STATE.sent_transactions.push(*pending_transaction);
        }
        Ok(pending_transaction)
    }

    async fn sign_transaction(
        &self,
        _: &TypedTransaction,
        _: Address,
    ) -> Result<Signature, Self::Error> {
        self.sign_transaction
            .ok_or(MockMiddlewareError::SignTransaction)
            .map(|_| Signature {
                r: u256(1),
                s: u256(1),
                v: 1,
            })
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
