use crate::actors::ActorManager;
use crate::error::*;
use crate::types::{self, ResubmitStrategy, TransactionProvider};

use offchain_utils::block_subscriber::NewBlockSubscriber;
use offchain_utils::middleware_factory::MiddlewareFactory;
use offchain_utils::offchain_core::ethers;

use ethers::types::{Address, U256};

use std::sync::Arc;

pub struct TransactionManager<ProviderFactory, BlockSubscriber, Label>
where
    ProviderFactory: types::ProviderFactory + 'static,
    BlockSubscriber: NewBlockSubscriber,
    Label: Eq + std::hash::Hash,
{
    provider_factory: Arc<ProviderFactory>,
    actor_manager: ActorManager<ProviderFactory, BlockSubscriber, Label>,
}

impl<PF, BS, L> TransactionManager<PF, BS, L>
where
    PF: types::ProviderFactory + Send + Sync + 'static,
    BS: NewBlockSubscriber + Send + Sync + 'static,
    L: Eq + std::hash::Hash + Clone + Send + Sync + 'static,
{
    pub fn new(
        provider_factory: Arc<PF>,
        block_subscriber: Arc<BS>,
        max_retries: usize,
        max_delay: std::time::Duration,
    ) -> Self {
        TransactionManager {
            provider_factory: Arc::clone(&provider_factory),
            actor_manager: ActorManager::new(
                provider_factory,
                block_subscriber,
                max_retries,
                max_delay,
            ),
        }
    }

    /// Gets the list of signer addresses.
    pub async fn accounts(
        &self,
    ) -> ProviderResult<
        Vec<Address>,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        let provider = self.provider_factory.get_provider(None).await?;
        provider.accounts().await
    }

    /// Gets the address for signer account with index `i`.
    pub async fn account(
        &self,
        i: usize,
    ) -> ProviderResult<
        Address,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        let provider = self.provider_factory.get_provider(None).await?;
        Ok(provider.accounts().await?[i])
    }

    /// Gets the latest balance for signer account with index `i`. For a more
    /// accurate balance, using the `StateFold` crate is recommended.
    pub async fn latest_balance(
        &self,
        i: usize,
    ) -> ProviderResult<
        U256,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        let address = self.account(i).await?;
        let provider = self.provider_factory.get_provider(None).await?;
        provider.balance(address).await
    }

    /// Returns if a transaction with a given label already exists.
    pub async fn label_exists(&self, label: &L) -> bool {
        self.actor_manager.label_exists(label).await
    }

    /// Starts the send transaction process. This will spawn a tokio thread,
    /// which will be responsible for watching the state of the submitted
    /// transaction, and for implementing some submit strategy. All the work is
    /// done in this background thread. All previously sent transaction's
    /// strategy get upgraded with the given strategy if needed.
    /// Returns false if is duplicate.
    pub async fn send_transaction<
        E: std::convert::Into<TxConversionError>,
        T: std::convert::TryInto<types::Transaction, Error = E>,
    >(
        &self,
        label: L,
        transaction: T,
        strategy: ResubmitStrategy,
        confirmations: usize,
    ) -> std::result::Result<bool, TxConversionError> {
        let t = transaction.try_into().map_err(|e| e.into())?;

        Ok(self
            .actor_manager
            .new_transaction(label, t, strategy, confirmations)
            .await)
    }

    /// Gets the latest transaction state. If the submit or invalidate
    /// background process has exited with an error, this function will return
    /// the error.
    pub async fn transaction_state(
        &self,
        label: &L,
    ) -> TransactionResult<
        types::TransactionState,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        self.actor_manager.get_state(label).await
    }

    /// Promotes the running transaction strategy with the given strategy. This
    /// function also promotes all transactions sent before the requested
    /// transaction.
    pub async fn promote_strategy(
        &self,
        label: &L,
        strategy: &ResubmitStrategy,
    ) -> TransactionResult<
        (),
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        self.actor_manager.promote_strategy(label, &strategy).await
    }

    /// Starts the invalidate transaction process, which will attempt to submit
    /// a transaction with no value and payload, to address zero, but with a
    /// higher gas and gas price than the original. This process is not
    /// guaranteed to invalidate the original transaction. It will spawn a tokio
    /// thread, which will be responsible for watching the state of the
    /// transaction, and for resubmitting with an increased gas price if needed.
    pub async fn invalidate_transaction(
        &self,
        label: &L,
    ) -> TransactionResult<
        (),
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        self.actor_manager.invalidate(label).await
    }

    /// If the background process has exited with an error, the user may ask
    /// for a retry. This function also retries all past failed transactions.
    pub async fn retry_transaction(
        &self,
        label: &L,
    ) -> TransactionResult<
        (),
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        self.actor_manager.retry(label).await
    }
}
