use crate::error::*;
use crate::types::{
    self, Transaction, TransactionReceipt, TransactionSubmission, TransferValue,
};

use offchain_utils::middleware_factory::MiddlewareFactory;
use offchain_utils::offchain_core::ethers;

use async_trait::async_trait;
use snafu::ResultExt;
use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::Arc;

use tokio::sync::{Mutex, OwnedMutexGuard};
use tokio::time::timeout;

use ethers::providers::Middleware;
use ethers::types::{
    Address, BlockId, BlockNumber, TransactionRequest, H256, U256,
};

/// Concrete web3 implementation of ProviderFactory
pub struct Factory<MF: MiddlewareFactory> {
    middleware_factory: Arc<MF>,
    call_timeout: std::time::Duration,
    nonce_mutex: Arc<NonceMutexManager>,
}

impl<MF: MiddlewareFactory> Factory<MF> {
    pub fn new(
        middleware_factory: Arc<MF>,
        call_timeout: std::time::Duration,
    ) -> Arc<Self> {
        Arc::new(Factory {
            middleware_factory,
            call_timeout,
            nonce_mutex: Arc::new(NonceMutexManager::new()),
        })
    }
}

pub struct NonceMutexManager {
    mutexes: Mutex<HashMap<Address, Arc<Mutex<()>>>>,
}

impl NonceMutexManager {
    fn new() -> Self {
        NonceMutexManager {
            mutexes: Mutex::new(HashMap::new()),
        }
    }

    async fn get_mutex(&self, address: &Address) -> Arc<Mutex<()>> {
        let mut mutexes = self.mutexes.lock().await;

        match mutexes.get(&address) {
            Some(mutex) => Arc::clone(mutex),
            None => {
                let mutex = Arc::new(Mutex::new(()));
                mutexes.insert(address.clone(), Arc::clone(&mutex));
                mutex
            }
        }
    }
}

#[async_trait]
impl<MF: MiddlewareFactory> types::ProviderFactory for Factory<MF>
where
    MF: Send + Sync,
{
    type MiddlewareFactory = MF;
    type Provider = Provider<MF::Middleware>;

    async fn get_provider(
        &self,
        previous: Option<Self::Provider>,
    ) -> ProviderResult<
        Self::Provider,
        <Self::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        let new_middleware = match &previous {
            Some(previous) => {
                self.middleware_factory
                    .new_middleware(Some(&previous.signer_middleware))
                    .await
            }
            None => self.middleware_factory.new_middleware(None).await,
        }
        .context(MiddlewareFactoryErr {})?;

        Ok(Provider::new(
            new_middleware,
            self.call_timeout,
            Arc::clone(&self.nonce_mutex),
        ))
    }
}

/// Concrete web3 implementation of TransactionActorProvider
pub struct Provider<M: Middleware> {
    signer_middleware: M,
    call_timeout: std::time::Duration,
    nonce_mutex: Arc<NonceMutexManager>,
}

impl<M: Middleware> Provider<M> {
    pub fn new(
        signer_middleware: M,
        call_timeout: std::time::Duration,
        nonce_mutex: Arc<NonceMutexManager>,
    ) -> Self {
        Provider {
            signer_middleware,
            call_timeout,
            nonce_mutex,
        }
    }
}

#[async_trait]
impl<M: Middleware> types::TransactionProvider for Provider<M> {
    type Middleware = M;

    async fn accounts(&self) -> ProviderResult<Vec<Address>, M> {
        self.signer_middleware
            .get_accounts()
            .await
            .context(EthersErr {
                err: "Error getting accounts",
            })
    }

    async fn lock_and_get_nonce(
        &self,
        address: Address,
    ) -> ProviderResult<(U256, OwnedMutexGuard<()>), M> {
        // Serialize concurrent `send`, to make sure we get correct the nonce.
        // We only drop the lock when we've submitted the transaction.
        let nonce_mutex = self.nonce_mutex.get_mutex(&address).await;
        let lock = nonce_mutex.lock_owned().await;

        let nonce = {
            let nonce_future = self.signer_middleware.get_transaction_count(
                address,
                Some(BlockId::Number(BlockNumber::Pending)),
            );

            timeout(self.call_timeout, nonce_future)
                .await
                .context(TimeoutErr {
                    err: "`lock_and_get_nonce` timeout while getting nonce",
                })?
                .context(EthersErr {
                    err: format!("Could not get nonce for addres {}", address),
                })?
        };

        Ok((nonce, lock))
    }

    async fn send(
        &self,
        transaction: &Transaction,
        gas: U256,
        gas_price: U256,
        nonce: U256,
    ) -> ProviderResult<TransactionSubmission, M> {
        let value = match transaction.value {
            TransferValue::Nothing => None,
            TransferValue::Value(v) => Some(v),

            TransferValue::All => {
                let balance = self.balance(transaction.from).await?;
                if balance > gas * gas_price {
                    Some(balance - gas * gas_price)
                } else {
                    // TODO: Out of funds. Is this the correct way to handle
                    // this?
                    None
                }
            }
        };

        let block_submitted = timeout(
            self.call_timeout,
            self.signer_middleware.get_block_number(),
        )
        .await
        .context(TimeoutErr {
            err: "`send` timeout while getting block number",
        })?
        .context(EthersErr {
            err: "Could not send get block number",
        })?;

        let hash = {
            let mut tx_request = TransactionRequest::default()
                .from(transaction.from)
                .to(transaction.to)
                .gas(gas)
                .gas_price(gas_price)
                .nonce(nonce);

            if let Some(v) = value {
                tx_request = tx_request.value(v);
            }

            if let Some(d) = &transaction.call_data {
                tx_request = tx_request.data(d.clone());
            }

            let send_future =
                self.signer_middleware.send_transaction(tx_request, None);

            timeout(self.call_timeout, send_future)
            .await
            .context(TimeoutErr {
                err: "`send` timeout while submitting transaction",
            })?
            .context(EthersErr {
                err: format!(
                    "Could not send transaction from {} to {} with payload {:?}",
                    transaction.from, transaction.to, transaction.call_data
                ),
            })?
        };

        Ok(TransactionSubmission {
            transaction: transaction.clone(),
            hash: *hash,
            nonce,
            value,
            gas,
            gas_price,
            block_submitted,
        })
    }

    async fn balance(&self, address: Address) -> ProviderResult<U256, M> {
        self.signer_middleware
            .get_balance(address, None)
            .await
            .context(EthersErr {
                err: format!("Could not get balance for address `{}`", address),
            })
    }

    async fn estimate_gas(
        &self,
        transaction: &Transaction,
    ) -> ProviderResult<U256, M> {
        let value = match transaction.value {
            TransferValue::Nothing => None,
            TransferValue::Value(v) => Some(v),

            // Not exact, but it's what we have for today. Using the entire
            // balance as value yields a insuficient funds for transaction
            // error.
            TransferValue::All => None,
        };

        let mut tx_request = TransactionRequest::default()
            .from(transaction.from)
            .to(transaction.to);

        if let Some(v) = value {
            tx_request = tx_request.value(v);
        }

        if let Some(d) = &transaction.call_data {
            tx_request = tx_request.data(d.clone());
        }

        let typed_tx = tx_request.into();
        let call_future = self.signer_middleware.estimate_gas(&typed_tx);

        timeout(self.call_timeout, call_future)
            .await
            .context(TimeoutErr {
                err: "`estimate_gas` timeout",
            })?
            .context(EthersErr {
                err: "Could not estimate gas",
            })
    }

    async fn gas_price(&self) -> ProviderResult<U256, M> {
        let call_future = self.signer_middleware.get_gas_price();

        timeout(self.call_timeout, call_future)
            .await
            .context(TimeoutErr {
                err: "`gas_price` timeout",
            })?
            .context(EthersErr {
                err: "Could not get gas price",
            })
    }

    async fn receipt(
        &self,
        hash: H256,
    ) -> ProviderResult<Option<TransactionReceipt>, M> {
        let call_future = self.signer_middleware.get_transaction_receipt(hash);
        let receipt = timeout(self.call_timeout, call_future)
            .await
            .context(TimeoutErr {
                err: "`receipt` timeout",
            })?
            .context(EthersErr {
                err: "Error getting receipt",
            })?;

        match receipt {
            Some(r) => {
                let r_converted = r
                    .try_into()
                    .map_err(|err| ReceiptIncomplete { err }.build())?;
                Ok(Some(r_converted))
            }
            None => Ok(None),
        }
    }
}
