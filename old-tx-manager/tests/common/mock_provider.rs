use tx_manager::error::*;
use tx_manager::types;

use offchain_utils::block_subscriber::NewBlockSubscriber;
use offchain_utils::middleware_factory::{
    self, MiddlewareFactory, PhantomFactory,
};
use offchain_utils::offchain_core::ethers;
use offchain_utils::offchain_core::types::Block;

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::{hash_map::Entry, HashMap};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, OwnedMutexGuard};

use ethers::providers::{JsonRpcClient, Middleware, Provider, ProviderError};
use ethers::types::{Address, Bloom, H256, U256, U64};

/// Mockchain
pub struct Mockchain {
    tx_pool: HashMap<Address, HashMap<U256, types::TransactionSubmission>>,
    last_nonce: HashMap<Address, U256>,
    tx_count: HashMap<Address, U256>,
    mined_transactions: HashMap<H256, types::TransactionReceipt>,

    block_count: U64,
    current_hash: H256,
    gas_price: U256,

    broadcast_new_block: broadcast::Sender<Block>,
    lock: Arc<Mutex<()>>,
}

impl Mockchain {
    pub fn new(gas_price: U256) -> Arc<Mutex<Self>> {
        let (tx, _) = broadcast::channel(16);
        Arc::new(Mutex::new(Mockchain {
            tx_pool: HashMap::new(),
            last_nonce: HashMap::new(),
            tx_count: HashMap::new(),
            mined_transactions: HashMap::new(),

            block_count: 0.into(),
            current_hash: H256::from_low_u64_le(0),
            gas_price,

            broadcast_new_block: tx,
            lock: Arc::new(Mutex::new(())),
        }))
    }

    pub fn add_transaction(
        &mut self,
        submission: types::TransactionSubmission,
    ) {
        self.tx_count
            .entry(submission.transaction.from)
            .and_modify(|x| *x += 1.into())
            .or_insert(1.into());

        let e = self
            .tx_pool
            .entry(submission.transaction.from)
            .or_insert(HashMap::new())
            .entry(submission.nonce);

        match e {
            Entry::Occupied(mut x) => {
                let val = x.get_mut();
                if submission.gas_price > val.gas_price {
                    *val = submission;
                }
            }

            Entry::Vacant(x) => {
                x.insert(submission);
            }
        }
    }

    pub fn mine_block(&mut self, price_threshold: U256) {
        self.block_count += U64::from(1);
        let parent_hash = self.current_hash;
        self.current_hash = H256::from_low_u64_le(self.block_count.as_u64());

        let mut i = 0;
        for (address, txs) in &self.tx_pool {
            let last_nonce =
                self.last_nonce.entry(*address).or_insert(0.into());

            loop {
                if let Some(s) = txs.get(last_nonce) {
                    if s.gas_price >= price_threshold {
                        let receipt = types::TransactionReceipt {
                            hash: s.hash,
                            index: i.into(),
                            block_hash: self.current_hash,
                            block_number: self.block_count,
                            gas_used: s.gas,
                            cumulative_gas_used: s.gas,
                            status: types::TransactionStatus::Succeded,
                        };
                        self.mined_transactions.insert(s.hash, receipt);

                        *last_nonce += 1.into();
                        i += 1;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        let block = Block {
            parent_hash,
            hash: self.current_hash,
            number: self.block_count,
            timestamp: 0.into(),
            logs_bloom: Bloom::zero(),
        };

        self.broadcast_new_block.send(block).unwrap();
    }

    pub fn set_price(&mut self, price: U256) {
        self.gas_price = price
    }

    pub fn gas_price(&self) -> U256 {
        self.gas_price
    }

    pub fn current_hash(&self) -> H256 {
        self.current_hash
    }

    pub fn current_block(&self) -> U64 {
        self.block_count
    }

    pub fn tx_count(&self, address: Address) -> U256 {
        self.tx_count
            .get(&address)
            .map(|x| x.clone())
            .unwrap_or(0.into())
    }

    pub async fn acquire_lock(&self) -> OwnedMutexGuard<()> {
        self.lock.clone().lock_owned().await
    }

    pub fn transaction(&self, h: H256) -> Option<types::TransactionReceipt> {
        self.mined_transactions.get(&h).map(|x| x.clone())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Block> {
        self.broadcast_new_block.subscribe()
    }
}

#[derive(Debug)]
pub struct DummyError {}
impl std::fmt::Display for DummyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Dummy Error")
    }
}
impl std::error::Error for DummyError {}
impl From<DummyError> for ProviderError {
    fn from(e: DummyError) -> Self {
        ProviderError::JsonRpcClientError(Box::new(e))
    }
}

#[derive(Debug)]
pub struct DummyRpcProvider {}

#[async_trait]
impl JsonRpcClient for DummyRpcProvider {
    type Error = DummyError;

    async fn request<T: Serialize + Send + Sync, R: DeserializeOwned>(
        &self,
        _: &str,
        _: T,
    ) -> std::result::Result<R, DummyError> {
        unreachable!()
    }
}

pub struct DummyMiddlewareFactory {}

#[async_trait]
impl MiddlewareFactory for DummyMiddlewareFactory {
    type Middleware = Arc<Provider<DummyRpcProvider>>;
    type InnerFactory = PhantomFactory<Provider<DummyRpcProvider>>;

    /// User implemented methods
    async fn current(&self) -> Self::Middleware {
        unreachable!("Dummy Middleware `current` unreachable")
    }

    async fn middleware_eq(&self, _: &Self::Middleware) -> bool {
        unreachable!("Dummy Middleware `middleware_eq` unreachable")
    }

    async fn inner_factory(&self) -> &Self::InnerFactory {
        unreachable!("Dummy Middleware `inner_factory` unreachable")
    }

    async fn build_and_set_middleware(
        &self,
        _: <Self::InnerFactory as MiddlewareFactory>::Middleware,
    ) -> Self::Middleware {
        unreachable!("Dummy Middleware `build_and_set_middleware` unreachable")
    }

    fn should_retry(_: &<Self::Middleware as Middleware>::Error) -> bool {
        unreachable!("Dummy Middleware `build_middleware` unreachable")
    }

    /// Default method
    async fn new_middleware(
        &self,
        _: Option<&Self::Middleware>,
    ) -> middleware_factory::Result<Self::Middleware> {
        Ok(Arc::new(Provider::new(DummyRpcProvider {})))
    }
}

/// Mock implementation of ProviderFactory
pub struct MockProviderFactory {
    mockchain: Arc<Mutex<Mockchain>>,
}

impl MockProviderFactory {
    pub fn new(mockchain: Arc<Mutex<Mockchain>>) -> Arc<Self> {
        Arc::new(MockProviderFactory { mockchain })
    }
}

#[async_trait]
impl types::ProviderFactory for MockProviderFactory {
    type MiddlewareFactory = DummyMiddlewareFactory;
    type Provider = MockProvider;

    async fn get_provider(
        &self,
        _previous: Option<Self::Provider>,
    ) -> ProviderResult<Self::Provider, Arc<Provider<DummyRpcProvider>>> {
        Ok(MockProvider::new(Arc::clone(&self.mockchain)))
    }
}

/// Mock implementation of TransactionProvider
pub struct MockProvider {
    mockchain: Arc<Mutex<Mockchain>>,
}

impl MockProvider {
    fn new(mockchain: Arc<Mutex<Mockchain>>) -> Self {
        MockProvider { mockchain }
    }
}

#[async_trait]
impl types::TransactionProvider for MockProvider {
    type Middleware = Arc<Provider<DummyRpcProvider>>;

    async fn accounts(
        &self,
    ) -> ProviderResult<Vec<Address>, Arc<Provider<DummyRpcProvider>>> {
        Ok(vec![Address::repeat_byte(1), Address::repeat_byte(2)])
    }

    async fn send(
        &self,
        transaction: &types::Transaction,
        gas: U256,
        gas_price: U256,
        nonce: U256,
    ) -> ProviderResult<
        types::TransactionSubmission,
        Arc<Provider<DummyRpcProvider>>,
    > {
        // Fake hash.
        let hash = H256::from_slice(&ethers::utils::keccak256(
            format!("{:?};{:?};{:?};{:?}", transaction, gas, gas_price, nonce)
                .as_bytes(),
        ));

        let submission = types::TransactionSubmission {
            transaction: transaction.clone(),
            hash,
            nonce,
            value: None,
            gas,
            gas_price,
            block_submitted: self.mockchain.lock().await.current_block(),
        };

        self.mockchain
            .lock()
            .await
            .add_transaction(submission.clone());

        Ok(submission)
    }

    async fn lock_and_get_nonce(
        &self,
        address: Address,
    ) -> ProviderResult<
        (U256, OwnedMutexGuard<()>),
        Arc<Provider<DummyRpcProvider>>,
    > {
        let nonce = self.mockchain.lock().await.tx_count(address);
        let lock = self.mockchain.lock().await.acquire_lock().await;
        Ok((nonce, lock))
    }

    async fn balance(
        &self,
        _address: Address,
    ) -> ProviderResult<U256, Arc<Provider<DummyRpcProvider>>> {
        unimplemented!()
    }

    async fn estimate_gas(
        &self,
        _transaction: &types::Transaction,
    ) -> ProviderResult<U256, Arc<Provider<DummyRpcProvider>>> {
        // Fake gas estimation.
        Ok(30_000.into())
    }

    async fn gas_price(
        &self,
    ) -> ProviderResult<U256, Arc<Provider<DummyRpcProvider>>> {
        Ok(self.mockchain.lock().await.gas_price())
    }

    async fn receipt(
        &self,
        hash: H256,
    ) -> ProviderResult<
        Option<types::TransactionReceipt>,
        Arc<Provider<DummyRpcProvider>>,
    > {
        Ok(self.mockchain.lock().await.transaction(hash))
    }
}

/// Mock implementation of NewBlockSubscriber
pub struct MockBlockSubscriber {
    mockchain: Arc<Mutex<Mockchain>>,
}

impl MockBlockSubscriber {
    pub fn new(mockchain: Arc<Mutex<Mockchain>>) -> Arc<Self> {
        Arc::new(MockBlockSubscriber { mockchain })
    }
}

#[async_trait]
impl NewBlockSubscriber for MockBlockSubscriber {
    async fn subscribe(&self) -> Option<broadcast::Receiver<Block>> {
        let mockchain = self.mockchain.lock().await;
        Some(mockchain.subscribe())
    }
}
