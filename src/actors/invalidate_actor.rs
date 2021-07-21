use super::ActorHandle;

use crate::error::*;
use crate::types::*;
use crate::utils;

use offchain_utils::backoff::Backoff;
use offchain_utils::block_subscriber::NewBlockSubscriber;
use offchain_utils::middleware_factory::MiddlewareFactory;
use offchain_utils::offchain_core::ethers;
use offchain_utils::offchain_core::types::Block;

use ethers::providers::Middleware;
use ethers::types::Address;

use snafu::ResultExt;
use std::sync::Arc;
use tokio::sync::{oneshot, watch};

pub(super) enum InvalidateActorRequest<M: Middleware + 'static> {
    TransactionHandle(ActorHandle<SendState, M>),
    LastState(InvalidateState),
}

pub(super) struct InvalidateActor<PF, BlockSubscriber>
where
    PF: ProviderFactory,
    BlockSubscriber: NewBlockSubscriber,
{
    provider_factory: Arc<PF>,
    block_subscriber: Arc<BlockSubscriber>,

    strategy_watch: watch::Receiver<ResubmitStrategy>,

    confirmations: usize,
    max_retries: usize,
    max_delay: std::time::Duration,
}

impl<PF, BS> InvalidateActor<PF, BS>
where
    PF: ProviderFactory + Send + Sync + 'static,
    BS: NewBlockSubscriber + Send + Sync + 'static,
{
    pub(super) fn run(
        provider_factory: Arc<PF>,
        block_subscriber: Arc<BS>,
        request: InvalidateActorRequest<
            <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
        >,
        strategy_watch: watch::Receiver<ResubmitStrategy>,
        confirmations: usize,
        max_retries: usize,
        max_delay: std::time::Duration,
    ) -> ActorHandle<
        InvalidateState,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        let this = InvalidateActor {
            provider_factory,
            block_subscriber,
            strategy_watch,
            confirmations,
            max_retries,
            max_delay,
        };

        this.start(request)
    }

    fn start(
        self,
        request: InvalidateActorRequest<
            <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
        >,
    ) -> ActorHandle<
        InvalidateState,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        let current_state = match &request {
            // Transaction actor processing transaction.
            InvalidateActorRequest::TransactionHandle(h) => {
                let state = h.state_watch.borrow().clone();
                InvalidateState::from(state)
            }

            // Ongoing invalidate, continue from where we left.
            InvalidateActorRequest::LastState(s) => s.clone(),
        };

        let (state_sender, state_receiver) = watch::channel(current_state);
        let (final_sender, final_receiver) = oneshot::channel();

        let handle = tokio::spawn(async move {
            self.background_process(request, state_sender, final_sender)
                .await
        });

        ActorHandle {
            handle,
            final_state: final_receiver,
            state_watch: state_receiver,
        }
    }

    async fn background_process(
        self,
        request: InvalidateActorRequest<
            <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
        >,
        state_sender: watch::Sender<InvalidateState>,
        final_sender: oneshot::Sender<FinalizedState>,
    ) -> WorkerResult<
        InvalidateState,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        // Get transaction's current state.
        let state = match request {
            // Continue invalidate process from where we left off.
            InvalidateActorRequest::LastState(r) => r,

            // Run TransactionActor to completion, taking its returned state.
            InvalidateActorRequest::TransactionHandle(mut h) => {
                // Drop channel, signaling actor should stop.
                drop(h.state_watch);

                // Take the actor's returned transaction state. This may
                // not return instantly.
                let ret = h.handle.await;
                let final_state = h.final_state.try_recv();

                // Convert into InvalidateState.
                let state = match ret {
                    Ok(Ok(s)) => InvalidateState::from(s),

                    // Error cases.
                    Ok(Err(e)) => return Err(e),
                    Err(e) => {
                        return Err(WorkerError::ActorJoinHandleErr {
                            source: e,
                        })
                    }
                };

                match final_state {
                    Ok(s) => {
                        let _ = final_sender.send(s);
                        return Ok(state);
                    }

                    Err(_) => state,
                }
            }
        };

        let original_submission = match state {
            InvalidateState::InvalidateRequested {
                original_submission: r,
            }
            | InvalidateState::InvalidateFailing {
                submission_receipt: SubmisssionReceipt { submission: r, .. },
                ..
            }
            | InvalidateState::Invalidating {
                original_submission: r,
                ..
            } => r,

            // TransactionActor shouldn't return a `Processing` variant.
            // Instantiating this actor with a Processing variant yeilds a
            // `Halted` state.
            InvalidateState::Processing => {
                let _ = final_sender.send(FinalizedState::Halted);
                return Ok(state);
            }
        };

        // Notify channel and return if channel has been dropped.
        let invalidate_state = InvalidateState::InvalidateRequested {
            original_submission: original_submission.clone(),
        };
        if state_sender.send(invalidate_state.clone()).is_err() {
            // TODO: Warn error.
            return Ok(invalidate_state);
        }

        // If we are retrying, there's no issue submitting a new invalidate
        // transaction. Only one of them will be mined.
        let invalidate_submission =
            self.invalidate(&original_submission).await?;

        self.wait_for_confirmations(
            original_submission,
            invalidate_submission,
            invalidate_state,
            &state_sender,
            final_sender,
        )
        .await
    }

    async fn invalidate(
        &self,
        original_submission: &TransactionSubmission,
    ) -> WorkerResult<
        TransactionSubmission,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        let mut provider = self.provider_factory.get_provider(None).await?;

        // Send transaction to the network, retrying if needed.
        let mut backoff = Backoff::new(self.max_retries, self.max_delay);
        let submission = loop {
            let result = self.send(original_submission, &provider).await;

            match result {
                Ok(s) => {
                    break s;
                }

                Err(e) if Self::should_retry(&e) => {
                    backoff.wait().await.map_err(|()| {
                        WorkerError::RetryLimitReachedErr { source: e }
                    })?;

                    provider = self
                        .provider_factory
                        .get_provider(Some(provider))
                        .await?;
                }

                Err(e) => {
                    return Err(e.into());
                }
            }
        };

        Ok(submission)
    }

    async fn send(
        &self,
        original_submission: &TransactionSubmission,
        provider: &PF::Provider,
    ) -> ProviderResult<
        TransactionSubmission,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        let transaction = Transaction {
            from: original_submission.transaction.from,
            to: Address::zero(),
            value: TransferValue::Nothing,
            call_data: None,
        };

        let gas = original_submission.gas;
        let gas_price = {
            let minimum_price = (original_submission.gas_price * 12) / 10;

            let current_price = {
                let price = provider.gas_price().await?;
                self.strategy_watch.borrow().multiply_gas_price(price)
            };

            std::cmp::max(minimum_price, current_price)
        };

        provider
            .send(&transaction, gas, gas_price, original_submission.nonce)
            .await
    }

    async fn wait_for_confirmations(
        &self,
        original_submission: TransactionSubmission,
        mut invalidate_submission: TransactionSubmission,
        mut current_state: InvalidateState,
        state_sender: &watch::Sender<InvalidateState>,
        final_sender: oneshot::Sender<FinalizedState>,
    ) -> WorkerResult<
        InvalidateState,
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

        loop {
            // Block on waiting for new block.
            let block = utils::get_last_block(&mut subscription).await?;

            let result = self
                .confirmation_step(
                    &provider,
                    block,
                    &original_submission,
                    &mut invalidate_submission,
                )
                .await;

            match result {
                Ok(InvalidateOrFinal::Invalidate(s)) => {
                    current_state = s.clone();
                    // Broadcast. If channel dropped, return current state.
                    if state_sender.send(s).is_err() {
                        return Ok(current_state);
                    };
                }

                Ok(InvalidateOrFinal::Final(s)) => {
                    let _ = final_sender.send(s);
                    return Ok(current_state);
                }

                Err(e) => provider = self.handle_err(provider, e).await?,
            }
        }
    }

    async fn confirmation_step(
        &self,
        provider: &PF::Provider,
        block: Block,
        original_submission: &TransactionSubmission,
        invalidate_submission: &mut TransactionSubmission,
    ) -> ProviderResult<
        InvalidateOrFinal,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        let (result_original, result_invalidate) = tokio::join!(
            provider.receipt(original_submission.hash),
            provider.receipt(invalidate_submission.hash),
        );

        match (result_original?, result_invalidate?) {
            // Failed to invalidate, original transaction confirmed.
            (Some(receipt), _)
                if receipt.block_number + self.confirmations < block.number =>
            {
                Ok(InvalidateOrFinal::Final(FinalizedState::Confirmed(receipt)))
            }

            // Transaction successfully invalidated.
            (_, Some(receipt))
                if receipt.block_number + self.confirmations < block.number =>
            {
                Ok(InvalidateOrFinal::Final(FinalizedState::Invalidated))
            }

            // Original transaction confirming.
            (Some(receipt), _) => {
                assert!(block.number >= receipt.block_number);
                let confirmations =
                    (block.number - receipt.block_number).as_usize();

                let state = InvalidateState::InvalidateFailing {
                    confirmations,
                    submission_receipt: SubmisssionReceipt {
                        submission: original_submission.clone(),
                        receipt,
                    },
                };

                Ok(InvalidateOrFinal::Invalidate(state))
            }

            // Transaction invalidating.
            (_, Some(receipt)) => {
                assert!(block.number >= receipt.block_number);
                let confirmations =
                    (block.number - receipt.block_number).as_usize();

                let state = InvalidateState::Invalidating {
                    confirmations,
                    original_submission: original_submission.clone(),
                };

                Ok(InvalidateOrFinal::Invalidate(state))
            }

            // Neither has been mined.
            (None, None) => {
                // Strategy. Resubmit if needed.
                let strategy = *self.strategy_watch.borrow();
                if invalidate_submission.block_submitted + strategy.rate
                    < block.number
                {
                    self.resubmit_if_higher(
                        provider,
                        invalidate_submission,
                        strategy,
                    )
                    .await?;
                }

                // Notify channel.
                let state = InvalidateState::InvalidateRequested {
                    original_submission: original_submission.clone(),
                };

                Ok(InvalidateOrFinal::Invalidate(state))
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
        if Self::should_retry(&err) {
            self.provider_factory.get_provider(Some(provider)).await
        } else {
            Err(err)
        }
    }

    fn should_retry(
        err: &ProviderError<
            <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
        >,
    ) -> bool {
        utils::should_retry::<PF::MiddlewareFactory>(err)
    }
}

enum InvalidateOrFinal {
    Invalidate(InvalidateState),
    Final(FinalizedState),
}
