use crate::error::*;
use crate::types::{
    self, ResubmitStrategy, TransactionProvider, TransactionSubmission,
};
use crate::utils;

use offchain_utils::backoff::Backoff;
use offchain_utils::middleware_factory::MiddlewareFactory;
use offchain_utils::offchain_core::ethers;

use ethers::types::U256;
use tokio::sync::watch;

pub struct SubmitWorker<'a, ProviderFactory>
where
    ProviderFactory: types::ProviderFactory,
{
    provider_factory: &'a ProviderFactory,
    transaction: &'a types::Transaction,
    strategy_watch: &'a watch::Receiver<ResubmitStrategy>,
    max_retries: usize,
    max_delay: std::time::Duration,
}

impl<'a, PF> SubmitWorker<'a, PF>
where
    PF: types::ProviderFactory + Send + Sync + 'static,
{
    pub async fn run(
        provider_factory: &'a PF,
        transaction: &'a types::Transaction,
        strategy_watch: &'a watch::Receiver<ResubmitStrategy>,
        max_retries: usize,
        max_delay: std::time::Duration,
    ) -> WorkerResult<
        TransactionSubmission,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        let this = SubmitWorker {
            provider_factory,
            transaction,
            strategy_watch,
            max_retries,
            max_delay,
        };

        this.start().await
    }

    async fn start(
        self,
    ) -> WorkerResult<
        TransactionSubmission,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        let mut provider = self.provider_factory.get_provider(None).await?;

        // Get nonce and mutex guard. Hold guard until transaction is submitted.
        let mut backoff = Backoff::new(self.max_retries, self.max_delay);
        let (nonce, _lock) = loop {
            // TODO: Possibly add a channel for cancelation here.

            let result =
                provider.lock_and_get_nonce(self.transaction.from).await;

            match result {
                Ok(x) => {
                    break x;
                }

                Err(e) if Self::should_retry(&e) => {
                    backoff.wait().await.map_err(|()| {
                        WorkerError::RetryLimitReachedErr { source: e }
                    })?;

                    provider = self
                        .provider_factory
                        .get_provider(Some(provider))
                        .await?;

                    continue;
                }

                Err(e) => {
                    return Err(e.into());
                }
            }
        };

        // Send transaction to the network, retrying if needed.
        let mut backoff = Backoff::new(self.max_retries, self.max_delay);
        loop {
            let result = self.send(&provider, nonce).await;

            match result {
                Ok(submission) => return Ok(submission),

                Err(e) if Self::should_retry(&e) => {
                    backoff.wait().await.map_err(|()| {
                        WorkerError::RetryLimitReachedErr { source: e }
                    })?;

                    provider = self
                        .provider_factory
                        .get_provider(Some(provider))
                        .await?;

                    continue;
                }

                Err(e) => {
                    return Err(e.into());
                }
            }
        }
    }

    async fn send(
        &self,
        provider: &PF::Provider,
        nonce: U256,
    ) -> ProviderResult<
        TransactionSubmission,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        let gas = {
            let g = provider.estimate_gas(&self.transaction).await?;
            self.strategy_watch.borrow().multiply_gas(g)
        };

        let gas_price = {
            let price = provider.gas_price().await?;
            self.strategy_watch.borrow().multiply_gas_price(price)
        };

        provider
            .send(&self.transaction, gas, gas_price, nonce)
            .await
    }

    fn should_retry(
        err: &ProviderError<
            <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
        >,
    ) -> bool {
        utils::should_retry::<PF::MiddlewareFactory>(err)
    }
}
