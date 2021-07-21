use super::strategy_worker::StrategyWorker;
use super::submit_worker::SubmitWorker;
use super::ActorHandle;

use crate::error::*;
use crate::types::{
    self, FinalizedState, ResubmitStrategy, SendState, SubmisssionReceipt,
};

use offchain_utils::block_subscriber::NewBlockSubscriber;
use offchain_utils::middleware_factory::MiddlewareFactory;

use std::sync::Arc;
use tokio::sync::{oneshot, watch};

pub(super) struct TransactionActor<ProviderFactory, BlockSubscriber>
where
    ProviderFactory: types::ProviderFactory,
    BlockSubscriber: NewBlockSubscriber,
{
    provider_factory: Arc<ProviderFactory>,
    block_subscriber: Arc<BlockSubscriber>,
    request: SendState,
    strategy_watch: watch::Receiver<ResubmitStrategy>,
    confirmations: usize,

    max_retries: usize,
    max_delay: std::time::Duration,
}

impl<PF, BS> TransactionActor<PF, BS>
where
    PF: types::ProviderFactory + Send + Sync + 'static,
    BS: NewBlockSubscriber + Send + Sync + 'static,
{
    pub(super) fn run(
        provider_factory: Arc<PF>,
        block_subscriber: Arc<BS>,
        request: SendState,
        strategy_watch: watch::Receiver<ResubmitStrategy>,
        confirmations: usize,
        max_retries: usize,
        max_delay: std::time::Duration,
    ) -> ActorHandle<
        SendState,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        let this = TransactionActor {
            provider_factory,
            block_subscriber,
            request,
            strategy_watch,
            confirmations,
            max_retries,
            max_delay,
        };

        this.start()
    }

    fn start(
        self,
    ) -> ActorHandle<
        SendState,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        let (watch_sender, watch_receiver) =
            watch::channel(self.request.clone());
        let (final_sender, final_receiver) = oneshot::channel();

        let handle = tokio::spawn(async move {
            self.background_process(watch_sender, final_sender).await
        });

        ActorHandle {
            handle,
            final_state: final_receiver,
            state_watch: watch_receiver,
        }
    }

    async fn background_process(
        &self,
        state_sender: watch::Sender<SendState>,
        final_sender: oneshot::Sender<FinalizedState>,
    ) -> WorkerResult<
        SendState,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        let (submission, state) = match &self.request {
            // Clean transaction request. Submit to blockchain and take
            // submission.
            SendState::Processing { transaction } => {
                let submission = SubmitWorker::run(
                    self.provider_factory.as_ref(),
                    &transaction,
                    &self.strategy_watch,
                    self.max_retries,
                    self.max_delay,
                )
                .await?;

                let state = SendState::Submitted {
                    submission: submission.clone(),
                };
                (submission, state)
            }

            // Ongoing transaction. Take already sent submission.
            SendState::Submitted { submission }
            | SendState::Confirming {
                submission_receipt: SubmisssionReceipt { submission, .. },
                ..
            } => (submission.clone(), self.request.clone()),
        };

        // Broadcast new state.
        if state_sender.send(state.clone()).is_err() {
            return Ok(state);
        }

        StrategyWorker::run(
            self.provider_factory.as_ref(),
            self.block_subscriber.as_ref(),
            submission,
            state,
            &self.strategy_watch,
            state_sender,
            final_sender,
            self.confirmations,
            self.max_retries,
        )
        .await
    }
}
