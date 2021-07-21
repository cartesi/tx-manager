use crate::error::*;
use crate::types::{
    self, FinalizedState, ResubmitStrategy, SendState, SubmisssionReceipt,
    TransactionProvider, TransactionSubmission,
};
use crate::utils;

use offchain_utils::block_subscriber::NewBlockSubscriber;
use offchain_utils::middleware_factory::MiddlewareFactory;
use offchain_utils::offchain_core::types::Block;

use snafu::ResultExt;
use tokio::sync::{oneshot, watch};

pub struct StrategyWorker<'a, ProviderFactory, BlockSubscriber>
where
    ProviderFactory: types::ProviderFactory,
    BlockSubscriber: NewBlockSubscriber,
{
    provider_factory: &'a ProviderFactory,
    block_subscriber: &'a BlockSubscriber,

    first_submission: TransactionSubmission,
    current_state: SendState,

    strategy_watch: &'a watch::Receiver<ResubmitStrategy>,
    state_sender: watch::Sender<SendState>,
    final_sender: oneshot::Sender<FinalizedState>,

    confirmations: usize,
    max_retries: usize,
}

impl<'a, PF, BS> StrategyWorker<'a, PF, BS>
where
    PF: types::ProviderFactory + Send + Sync + 'static,
    BS: NewBlockSubscriber + Send + Sync + 'static,
{
    pub async fn run(
        provider_factory: &'a PF,
        block_subscriber: &'a BS,
        submission: TransactionSubmission,
        current_state: SendState,
        strategy_watch: &'a watch::Receiver<ResubmitStrategy>,
        state_sender: watch::Sender<SendState>,
        final_sender: oneshot::Sender<FinalizedState>,
        confirmations: usize,
        max_retries: usize,
    ) -> WorkerResult<
        SendState,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        let this = StrategyWorker {
            provider_factory,
            block_subscriber,
            first_submission: submission,
            current_state,
            strategy_watch,
            state_sender,
            final_sender,
            confirmations,
            max_retries,
        };

        this.wait_for_confirmations().await
    }

    async fn wait_for_confirmations(
        mut self,
    ) -> WorkerResult<
        SendState,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        // Start subscribe to new blocks. From now on, all new blocks will be
        // added to the stream's queue, to be consumed by calls to `next`.
        let mut subscription = self
            .block_subscriber
            .subscribe()
            .await
            .ok_or(snafu::NoneError)
            .context(SubscriberDroppedErr)?;

        let mut provider = self.provider_factory.get_provider(None).await?;
        let mut submission = self.first_submission.clone();

        let mut retry = 0;
        loop {
            // Block on waiting for new block.
            let block = utils::get_last_block(&mut subscription).await?;

            let result = self
                .confirmation_step(&provider, block, &mut submission)
                .await;

            match result {
                Ok(SendOrFinal::Sending(s)) => {
                    self.current_state = s.clone();
                    // Broadcast. If channel dropped, return current state.
                    if self.state_sender.send(s).is_err() {
                        return Ok(self.current_state);
                    };
                }

                Ok(SendOrFinal::Final(s)) => {
                    let _ = self.final_sender.send(s);
                    return Ok(self.current_state);
                }

                Err(e) => {
                    retry += 1;
                    if retry > self.max_retries {
                        return Err(WorkerError::RetryLimitReachedErr {
                            source: e,
                        });
                    }
                    provider = self.handle_err(provider, e).await?;
                    continue;
                }
            }

            retry = 0;
        }
    }

    async fn confirmation_step(
        &self,
        provider: &PF::Provider,
        block: Block,
        submission: &mut TransactionSubmission,
    ) -> ProviderResult<
        SendOrFinal,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        let receipt = provider.receipt(submission.hash).await?;

        match receipt {
            // Transaction confirmed.
            Some(receipt)
                if receipt.block_number + self.confirmations
                    <= block.number =>
            {
                Ok(SendOrFinal::Final(FinalizedState::Confirmed(receipt)))
            }

            // Transaction confirming.
            Some(receipt) => {
                assert!(block.number >= receipt.block_number);
                let confirmations =
                    (block.number - receipt.block_number).as_usize();

                Ok(SendOrFinal::Sending(SendState::Confirming {
                    confirmations,
                    submission_receipt: SubmisssionReceipt {
                        receipt,
                        submission: submission.clone(),
                    },
                }))
            }

            // Transaction not mined.
            None => {
                // Strategy. Resubmit if needed.
                let strategy = *self.strategy_watch.borrow();
                if submission.block_submitted + strategy.rate < block.number {
                    self.resubmit_if_higher(provider, submission, strategy)
                        .await?;
                }

                Ok(SendOrFinal::Sending(SendState::Submitted {
                    submission: submission.clone(),
                }))
            }
        }
    }

    async fn resubmit_if_higher(
        &self,
        provider: &PF::Provider,
        submission: &mut TransactionSubmission,
        strategy: ResubmitStrategy,
    ) -> ProviderResult<
        (),
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        let gas_price = {
            let price = provider.gas_price().await?;
            strategy.multiply_gas_price(price)
        };

        if gas_price <= submission.gas_price {
            return Ok(());
        }

        *submission = provider
            .send(
                &submission.transaction,
                submission.gas,
                gas_price,
                submission.nonce,
            )
            .await?;

        Ok(())
    }

    async fn handle_err(
        &self,
        provider: PF::Provider,
        err: ProviderError<
            <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
        >,
    ) -> ProviderResult<
        PF::Provider,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        if utils::should_retry::<PF::MiddlewareFactory>(&err) {
            self.provider_factory.get_provider(Some(provider)).await
        } else {
            Err(err)
        }
    }
}

enum SendOrFinal {
    Sending(SendState),
    Final(FinalizedState),
}
