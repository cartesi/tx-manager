use async_trait::async_trait;
use ethers::providers::{
    FromErr, Middleware, MockProvider, PendingTransaction, Provider,
};
use ethers::signers::{LocalWallet, Signer};
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::{
    Address, Block, BlockId, NameOrAddress, Signature, TransactionReceipt,
    TxHash, U256, U64,
};
use ethers::utils::keccak256;
use std::cmp::max;

// Middleware mock.

#[derive(Debug, thiserror::Error)]
pub enum MockMiddlewareError {
    #[error("mock middleware error: estimate gas")]
    EstimateGas,

    #[error("mock middleware error: get block")]
    GetBlock,

    #[error("mock middleware error: get block number")]
    GetBlockNumber,

    #[error("mock middleware error: estimate EIP1559 fees")]
    EstimateEIP1559Fees,

    #[error("mock middleware error: get transaction count")]
    GetTransactionCount,

    #[error("mock middleware error: get transaction receipt ($0)")]
    GetTransactionReceipt(i32),

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

#[derive(Debug)]
pub struct MockMiddleware {
    provider: (Provider<MockProvider>, MockProvider),
    pub estimate_gas: Option<U256>,
    pub get_block: Option<()>,
    pub get_block_number: Vec<u32>,
    pub estimate_eip1559_fees: Option<(u32, u32)>,
    pub get_transaction_count: Option<()>,
    pub get_transaction_receipt: Vec<bool>,
    pub send_transaction: Option<()>,
    pub sign_transaction: Option<()>,
}

impl MockMiddleware {
    pub fn new() -> Self {
        unsafe {
            GLOBAL = Global::default();
        }
        Self {
            provider: Provider::mocked(),
            estimate_gas: None,
            get_block: None,
            get_block_number: Vec::new(),
            estimate_eip1559_fees: None,
            get_transaction_count: None,
            get_transaction_receipt: Vec::new(),
            send_transaction: None,
            sign_transaction: None,
        }
    }

    pub fn global() -> &'static Global {
        unsafe { &GLOBAL }
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
        unsafe {
            GLOBAL.estimate_gas_n += 1;
        }
        self.estimate_gas.ok_or(MockMiddlewareError::EstimateGas)
    }

    async fn get_block<T: Into<BlockId> + Send + Sync>(
        &self,
        _: T,
    ) -> Result<Option<Block<TxHash>>, Self::Error> {
        unsafe {
            GLOBAL.get_block_n += 1;
        }
        let mut block = Block::<TxHash>::default();
        block.base_fee_per_gas = Some(u256(250));
        match self.get_block {
            None => Err(MockMiddlewareError::GetBlock),
            Some(_) => Ok(Some(block)),
        }
    }

    async fn get_block_number(&self) -> Result<U64, Self::Error> {
        let i = unsafe { GLOBAL.get_block_number_n as usize };
        unsafe {
            GLOBAL.get_block_number_n += 1;
        };
        if i >= self.get_block_number.len() {
            Err(MockMiddlewareError::GetBlockNumber)
        } else {
            Ok(u64(self.get_block_number[i]))
        }
    }

    async fn estimate_eip1559_fees(
        &self,
        _: Option<fn(U256, Vec<Vec<U256>>) -> (U256, U256)>,
    ) -> Result<(U256, U256), Self::Error> {
        unsafe {
            GLOBAL.estimate_eip1559_fees_n += 1;
        };
        self.estimate_eip1559_fees
            .map(|(x, y)| (U256::from(x), U256::from(y)))
            .ok_or(MockMiddlewareError::EstimateEIP1559Fees)
    }

    async fn get_transaction_count<T: Into<NameOrAddress> + Send + Sync>(
        &self,
        _: T,
        _: Option<BlockId>,
    ) -> Result<U256, Self::Error> {
        unsafe {
            GLOBAL.get_transaction_count_n += 1;
        }
        self.get_transaction_count
            .ok_or(MockMiddlewareError::GetTransactionCount)?;
        unsafe { Ok(u256(GLOBAL.nonce)) }
    }

    async fn get_transaction_receipt<T: Send + Sync + Into<TxHash>>(
        &self,
        transaction_hash: T,
    ) -> Result<Option<TransactionReceipt>, Self::Error> {
        let i = unsafe { GLOBAL.get_transaction_receipt_n as usize };
        unsafe {
            GLOBAL.get_transaction_receipt_n += 1;
        }
        if i >= self.get_transaction_receipt.len() {
            return Err(MockMiddlewareError::GetTransactionReceipt(i as i32));
        }

        if !self.get_transaction_receipt[i] {
            Ok(None)
        } else {
            let mut receipt = TransactionReceipt::default();
            receipt.transaction_hash = transaction_hash.into();

            let j = unsafe { GLOBAL.get_block_number_n as usize } - 1;
            let block_number = max(0, self.get_block_number[j]);
            receipt.block_number = Some(u64(block_number));
            Ok(Some(receipt))
        }
    }

    async fn send_transaction<T: Into<TypedTransaction> + Send + Sync>(
        &self,
        tx: T,
        _: Option<BlockId>,
    ) -> Result<PendingTransaction<'_, Self::Provider>, Self::Error> {
        unsafe {
            GLOBAL.send_transaction_n += 1;
            GLOBAL.sign_transaction_n -= 1;
        }
        let tx: &TypedTransaction = &tx.into();
        let signature = self.sign_transaction(tx, *tx.from().unwrap()).await?;
        let bytes = tx.rlp_signed(U64::from(1), &signature);
        let hash = self
            .send_transaction
            .map(|_| TxHash(keccak256(bytes)))
            .ok_or(MockMiddlewareError::SendTransaction)?;

        let pending_transaction =
            PendingTransaction::new(hash, self.provider());
        unsafe {
            GLOBAL.sent_transactions.push(*pending_transaction);
        }
        Ok(pending_transaction)
    }

    async fn sign_transaction(
        &self,
        tx: &TypedTransaction,
        _: Address,
    ) -> Result<Signature, Self::Error> {
        unsafe {
            GLOBAL.sign_transaction_n += 1;
        }
        let signer: LocalWallet =
            "380eb0f3d505f087e438eca80bc4df9a7faa24f868e69fc0440261a0fc0567dc"
                .parse()
                .unwrap();
        let signature = signer.sign_transaction(tx).await.unwrap();
        self.sign_transaction
            .ok_or(MockMiddlewareError::SignTransaction)
            .map(|_| signature)
    }
}

fn u64(n: u32) -> U64 {
    U64::from_dec_str(&n.to_string()).unwrap()
}

fn u256(n: u32) -> U256 {
    U256::from_dec_str(&n.to_string()).unwrap()
}

// Global state used to simulate the blockchain.

pub struct Global {
    pub nonce: u32,
    pub sent_transactions: Vec<TxHash>,

    // Stores how many times each function was called.
    pub estimate_gas_n: i32,
    pub get_block_n: i32,
    pub get_block_number_n: i32,
    pub estimate_eip1559_fees_n: i32,
    pub get_transaction_count_n: i32,
    pub get_transaction_receipt_n: i32,
    pub send_transaction_n: i32,
    pub sign_transaction_n: i32,
}

static mut GLOBAL: Global = Global::default();

impl Global {
    const fn default() -> Global {
        Global {
            nonce: 1,
            sent_transactions: Vec::new(),
            estimate_gas_n: 0,
            get_block_n: 0,
            get_block_number_n: 0,
            estimate_eip1559_fees_n: 0,
            get_transaction_count_n: 0,
            get_transaction_receipt_n: 0,
            send_transaction_n: 0,
            sign_transaction_n: 0,
        }
    }
}
