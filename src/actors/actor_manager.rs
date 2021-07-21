use super::invalidate_actor::{InvalidateActor, InvalidateActorRequest};
use super::transaction_actor::TransactionActor;
use super::ActorHandle;

use crate::error::*;
use crate::types::{
    self, FinalizedState, InvalidateState, ProviderFactory, ResubmitStrategy,
    SendState, TransactionState,
};

use offchain_utils::block_subscriber::NewBlockSubscriber;
use offchain_utils::middleware_factory::MiddlewareFactory;
use offchain_utils::offchain_core::ethers;

use ethers::providers::Middleware;
use ethers::types::Address;

use snafu::ResultExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, watch, Mutex};

pub struct ActorManager<ProviderFactory, BlockSubscriber, Label>
where
    ProviderFactory: types::ProviderFactory + 'static,
    BlockSubscriber: NewBlockSubscriber,
    Label: Eq + std::hash::Hash,
{
    provider_factory: Arc<ProviderFactory>,
    block_subscriber: Arc<BlockSubscriber>,

    current_id: Mutex<usize>,
    running_actors: Mutex<HashMap<Label, ActorCtx<ProviderFactory>>>,
    finalized_transactions: Mutex<HashMap<Label, FinalizedState>>,

    max_retries: usize,
    max_delay: std::time::Duration,
}

impl<PF, BS, L> ActorManager<PF, BS, L>
where
    PF: types::ProviderFactory + Send + Sync + 'static,
    BS: NewBlockSubscriber + Send + Sync + 'static,
    L: Eq + std::hash::Hash + Send + Sync + 'static,
{
    pub fn new(
        provider_factory: Arc<PF>,
        block_subscriber: Arc<BS>,
        max_retries: usize,
        max_delay: std::time::Duration,
    ) -> Self {
        ActorManager {
            provider_factory,
            block_subscriber,
            current_id: Mutex::new(0),
            running_actors: Mutex::new(HashMap::new()),
            finalized_transactions: Mutex::new(HashMap::new()),
            max_retries,
            max_delay,
        }
    }

    pub async fn label_exists(&self, label: &L) -> bool {
        self.running_actors.lock().await.contains_key(label)
            || self.finalized_transactions.lock().await.contains_key(label)
    }

    pub async fn new_transaction(
        &self,
        label: L,
        transaction: types::Transaction,
        strategy: ResubmitStrategy,
        confirmations: usize,
    ) -> bool {
        // Deduplicate.
        if self.label_exists(&label).await {
            return false;
        }

        // Promote all previous running transactions and retry them if needed.
        let mut actors = self.running_actors.lock().await;
        let old_actors = {
            let capacity = actors.capacity();
            std::mem::replace(&mut *actors, HashMap::with_capacity(capacity))
        };
        for (l, a) in old_actors {
            // Promote and retry actor strategy.
            let status = a
                .retry(
                    Arc::clone(&self.provider_factory),
                    Arc::clone(&self.block_subscriber),
                    Some(&strategy),
                    self.max_retries,
                    self.max_delay,
                    true,
                )
                .await;

            // Insert back to correct set.
            match status {
                ActorStatus::Running(a) => {
                    // Insert it back.
                    actors.insert(l, a);
                }

                ActorStatus::Finished(f) => {
                    // Insert it back.
                    self.finalized_transactions.lock().await.insert(l, f);
                }
            }
        }

        let sender_address = transaction.from;
        let (tx, rx) = watch::channel(strategy);

        // Start `TransactionActor`.
        let handle = TransactionActor::run(
            Arc::clone(&self.provider_factory),
            Arc::clone(&self.block_subscriber),
            SendState::Processing { transaction },
            rx.clone(),
            confirmations,
            self.max_retries,
            self.max_delay,
        );

        // Get new id and increment last id.
        let id = {
            let mut lock = self.current_id.lock().await;
            let id = *lock;
            *lock += 1;
            id
        };

        // Add to actors set.
        let entry = ActorCtx {
            sender_address,
            id,
            state: ActorState::Transaction(handle),
            confirmations,
            strategy_sender: tx,
            strategy_receiver: rx,
        };

        actors.insert(label, entry);
        true
    }

    pub async fn promote_strategy(
        &self,
        label: &L,
        strategy: &ResubmitStrategy,
    ) -> TransactionResult<
        (),
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        if self.finalized_transactions.lock().await.contains_key(label) {
            return Ok(());
        }

        // Lock mutex and get actor.
        let mut actors = self.running_actors.lock().await;
        let (l, actor) = actors
            .remove_entry(label)
            .ok_or(snafu::NoneError)
            .context(TxNonexistentErr {})?;

        // Promote current actor.
        let new_actor = match actor.promote_strategy(strategy).await {
            ActorStatus::Running(a) => a,

            ActorStatus::Finished(f) => {
                // Insert it back.
                self.finalized_transactions.lock().await.insert(l, f);
                return Ok(());
            }
        };

        // Promote all transactions with id smaller than actor, and retry them
        // if needed.
        let old_actors = {
            let capacity = actors.capacity();
            std::mem::replace(&mut *actors, HashMap::with_capacity(capacity))
        };
        for (l, a) in old_actors {
            if a.sender_address == new_actor.sender_address
                && a.id < new_actor.id
            {
                // Promote and retry actor strategy.
                let status = a
                    .retry(
                        Arc::clone(&self.provider_factory),
                        Arc::clone(&self.block_subscriber),
                        Some(&strategy),
                        self.max_retries,
                        self.max_delay,
                        true,
                    )
                    .await;

                // Insert it back.
                match status {
                    ActorStatus::Running(a) => {
                        actors.insert(l, a);
                    }

                    ActorStatus::Finished(f) => {
                        self.finalized_transactions.lock().await.insert(l, f);
                    }
                }
            } else {
                // Insert it back.
                actors.insert(l, a);
            }
        }

        // Add actor state back to set, and return transaction state.
        actors.insert(l, new_actor);
        Ok(())
    }

    pub async fn get_state(
        &self,
        label: &L,
    ) -> TransactionResult<
        types::TransactionState,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        if let Some(f) = self.finalized_transactions.lock().await.get(label) {
            return Ok(TransactionState::Finalized(f.clone()));
        }

        // Lock mutex and get actor.
        let mut actors = self.running_actors.lock().await;
        let (l, actor) = actors
            .remove_entry(label)
            .ok_or(snafu::NoneError)
            .context(TxNonexistentErr {})?;

        // Advance actor state, and get transaction state.
        let (new_actor, state) = actor.get_state().await;

        // Insert it back.
        match new_actor {
            ActorStatus::Running(a) => {
                actors.insert(l, a);
            }
            ActorStatus::Finished(f) => {
                self.finalized_transactions.lock().await.insert(l, f);
            }
        }

        state
    }

    pub async fn invalidate(
        &self,
        label: &L,
    ) -> TransactionResult<
        (),
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        if self.finalized_transactions.lock().await.contains_key(label) {
            return Ok(());
        }

        // Lock mutex and get actor.
        let mut actors = self.running_actors.lock().await;
        let (l, actor) = actors
            .remove_entry(label)
            .ok_or(snafu::NoneError)
            .context(TxNonexistentErr {})?;

        // Make invalidate actor state transition.
        let new_actor = actor
            .invalidate(
                Arc::clone(&self.provider_factory),
                Arc::clone(&self.block_subscriber),
                self.max_retries,
                self.max_delay,
            )
            .await;

        // Insert it back.
        match new_actor {
            ActorStatus::Running(a) => {
                actors.insert(l, a);
            }
            ActorStatus::Finished(f) => {
                self.finalized_transactions.lock().await.insert(l, f);
            }
        }

        Ok(())
    }

    pub async fn retry(
        &self,
        label: &L,
    ) -> TransactionResult<
        (),
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        if self.finalized_transactions.lock().await.contains_key(label) {
            return Ok(());
        }

        // Lock mutex and get actor.
        let mut actors = self.running_actors.lock().await;
        let (l, actor) = actors
            .remove_entry(label)
            .ok_or(snafu::NoneError)
            .context(TxNonexistentErr {})?;

        // Retry current actor.
        let new_actor = match actor
            .retry(
                Arc::clone(&self.provider_factory),
                Arc::clone(&self.block_subscriber),
                None,
                self.max_retries,
                self.max_delay,
                false,
            )
            .await
        {
            ActorStatus::Running(a) => a,
            ActorStatus::Finished(f) => {
                // Insert it back.
                self.finalized_transactions.lock().await.insert(l, f);
                return Ok(());
            }
        };

        // Retry all transactions with id smaller than actor.
        let old_actors = {
            let capacity = actors.capacity();
            std::mem::replace(&mut *actors, HashMap::with_capacity(capacity))
        };
        for (l, a) in old_actors {
            if a.sender_address == new_actor.sender_address
                && a.id < new_actor.id
            {
                // Promote and retry actor strategy.
                let status = a
                    .retry(
                        Arc::clone(&self.provider_factory),
                        Arc::clone(&self.block_subscriber),
                        None,
                        self.max_retries,
                        self.max_delay,
                        true,
                    )
                    .await;

                // Insert it back.
                match status {
                    ActorStatus::Running(a) => {
                        actors.insert(l, a);
                    }

                    ActorStatus::Finished(f) => {
                        self.finalized_transactions.lock().await.insert(l, f);
                    }
                }
            } else {
                // Insert it back.
                actors.insert(l, a);
            }
        }

        // Add actor state back to set.
        actors.insert(l, new_actor);
        Ok(())
    }
}

enum ActorStatus<PF: ProviderFactory + 'static> {
    Running(ActorCtx<PF>),
    Finished(FinalizedState),
}

struct ActorCtx<PF: ProviderFactory + 'static> {
    sender_address: Address,
    state: ActorState<PF>,
    id: usize,

    strategy_sender: watch::Sender<ResubmitStrategy>,
    strategy_receiver: watch::Receiver<ResubmitStrategy>,

    confirmations: usize,
}

impl<PF: ProviderFactory> ActorCtx<PF>
where
    PF: Send + Sync + 'static,
{
    async fn promote_strategy(
        mut self,
        strategy: &ResubmitStrategy,
    ) -> ActorStatus<PF> {
        self.state = match self.state.advance_state().await {
            ActorState::Finalized(f) => return ActorStatus::Finished(f),
            x => x,
        };

        let old_strategy = *self.strategy_receiver.borrow();
        let new_strategy = old_strategy.join(&strategy);
        let _ = self.strategy_sender.send(new_strategy);

        ActorStatus::Running(self)
    }

    async fn get_state(
        mut self,
    ) -> (
        ActorStatus<PF>,
        TransactionResult<
            TransactionState,
            <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
        >,
    ) {
        self.state = match self.state.advance_state().await {
            ActorState::Finalized(f) => {
                return (
                    ActorStatus::Finished(f.clone()),
                    Ok(TransactionState::Finalized(f)),
                )
            }
            x => x,
        };

        let state = self.state.get_transaction_state().await;
        (ActorStatus::Running(self), state)
    }

    async fn invalidate<BS>(
        mut self,
        provider_factory: Arc<PF>,
        block_subscriber: Arc<BS>,
        max_retries: usize,
        max_delay: std::time::Duration,
    ) -> ActorStatus<PF>
    where
        BS: NewBlockSubscriber + Send + Sync + 'static,
    {
        self.state = match self
            .state
            .invalidate_state(
                provider_factory,
                block_subscriber,
                self.strategy_receiver.clone(),
                self.confirmations,
                max_retries,
                max_delay,
            )
            .await
        {
            ActorState::Finalized(f) => return ActorStatus::Finished(f),
            x => x,
        };

        ActorStatus::Running(self)
    }

    async fn retry<BS>(
        mut self,
        provider_factory: Arc<PF>,
        block_subscriber: Arc<BS>,
        strategy: Option<&ResubmitStrategy>,
        max_retries: usize,
        max_delay: std::time::Duration,
        submitted_only: bool,
    ) -> ActorStatus<PF>
    where
        BS: NewBlockSubscriber + Send + Sync + 'static,
    {
        self.state = match self
            .state
            .retry_state(
                provider_factory,
                block_subscriber,
                self.strategy_receiver.clone(),
                self.confirmations,
                max_retries,
                max_delay,
                submitted_only,
            )
            .await
        {
            ActorState::Finalized(f) => return ActorStatus::Finished(f),
            x => x,
        };

        if let Some(strategy) = strategy {
            let old_strategy = *self.strategy_receiver.borrow();
            let new_strategy = old_strategy.join(&strategy);
            let _ = self.strategy_sender.send(new_strategy);
        }

        ActorStatus::Running(self)
    }
}

enum ActorState<PF: ProviderFactory + 'static> {
    Transaction(
        ActorHandle<
            SendState,
            <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
        >,
    ),

    Invalidate(
        ActorHandle<
            InvalidateState,
            <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
        >,
    ),

    Finalized(FinalizedState),

    TransactionErr(
        SendState,
        Arc<
            WorkerError<
                <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
            >,
        >,
    ),

    InvalidateErr(
        InvalidateState,
        Arc<
            WorkerError<
                <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
            >,
        >,
    ),
}

impl<PF: ProviderFactory> ActorState<PF>
where
    PF: Send + Sync + 'static,
{
    async fn advance_state(self) -> Self {
        match self {
            // These states cannot be advanced. They yield the same state they
            // were in before.
            ActorState::Finalized(_)
            | ActorState::TransactionErr(_, _)
            | ActorState::InvalidateErr(_, _) => self,

            // Check if `TransactionActor` has finished running, moving either
            // to `TransactionErr` (if it finished with an error), `Finalized`
            // (if it finished Ok), or `Transaction` (if it's still running)
            // states.
            ActorState::Transaction(handle) => {
                match try_actor_ready(handle).await {
                    TryActorReady::Ready(state) => ActorState::Finalized(state),
                    TryActorReady::Pending(handle) => {
                        ActorState::Transaction(handle)
                    }
                    TryActorReady::Error(s, e) => {
                        ActorState::TransactionErr(s, e)
                    }
                }
            }

            // Check if `InvalidateActor` has finished running, moving either
            // to `InvalidateErr` (if it finished with an error), `Finalized`
            // (if it finished Ok), or `Invalidate` (if it's still running)
            // states.
            ActorState::Invalidate(handle) => {
                match try_actor_ready(handle).await {
                    TryActorReady::Ready(state) => ActorState::Finalized(state),
                    TryActorReady::Pending(handle) => {
                        ActorState::Invalidate(handle)
                    }
                    TryActorReady::Error(s, e) => {
                        ActorState::InvalidateErr(s, e)
                    }
                }
            }
        }
    }

    async fn invalidate_state<BS>(
        self,
        provider_factory: Arc<PF>,
        block_subscriber: Arc<BS>,
        strategy_receiver: watch::Receiver<ResubmitStrategy>,
        confirmations: usize,
        max_retries: usize,
        max_delay: std::time::Duration,
    ) -> Self
    where
        BS: NewBlockSubscriber + Send + Sync + 'static,
    {
        // First advance the state.
        let new_state = self.advance_state().await;
        match new_state {
            // If it's finalized, or invalidate is already running, there's
            // nothing to do.
            ActorState::Finalized(_) | ActorState::Invalidate(_) => new_state,

            // If `TransactionActor` is running, we move to invalidate. Start
            // the `InvalidateActor`, and change the actor state to
            // `Invalidate`.
            ActorState::Transaction(handle) => {
                ActorState::Invalidate(InvalidateActor::run(
                    provider_factory,
                    block_subscriber,
                    InvalidateActorRequest::TransactionHandle(handle),
                    strategy_receiver,
                    confirmations,
                    max_retries,
                    max_delay,
                ))
            }

            // If it's in an `TransactionErr` state, we start the
            // `InvalidateActor`, converting its last state to
            // `InvalidateState` and passing it to the actor.
            ActorState::TransactionErr(s, _) => {
                ActorState::Invalidate(InvalidateActor::run(
                    provider_factory,
                    block_subscriber,
                    InvalidateActorRequest::LastState(InvalidateState::from(s)),
                    strategy_receiver,
                    confirmations,
                    max_retries,
                    max_delay,
                ))
            }

            // If it's in an `InvalidateErr` state, retry the `InvalidateActor`.
            ActorState::InvalidateErr(s, _) => {
                ActorState::Invalidate(InvalidateActor::run(
                    provider_factory,
                    block_subscriber,
                    InvalidateActorRequest::LastState(s),
                    strategy_receiver,
                    confirmations,
                    max_retries,
                    max_delay,
                ))
            }
        }
    }

    async fn retry_state<BS>(
        self,
        provider_factory: Arc<PF>,
        block_subscriber: Arc<BS>,
        strategy_receiver: watch::Receiver<ResubmitStrategy>,
        confirmations: usize,
        max_retries: usize,
        max_delay: std::time::Duration,
        submitted_only: bool,
    ) -> Self
    where
        BS: NewBlockSubscriber + Send + Sync + 'static,
    {
        let new_state = self.advance_state().await;
        match new_state {
            ActorState::Finalized(_)
            | ActorState::Transaction(_)
            | ActorState::Invalidate(_) => new_state,

            ActorState::TransactionErr(s, e) => {
                if submitted_only {
                    if let SendState::Processing { .. } = &s {
                        return ActorState::TransactionErr(s, e);
                    }
                }

                ActorState::Transaction(TransactionActor::run(
                    provider_factory,
                    block_subscriber,
                    s,
                    strategy_receiver,
                    confirmations,
                    max_retries,
                    max_delay,
                ))
            }

            ActorState::InvalidateErr(s, _) => {
                ActorState::Invalidate(InvalidateActor::run(
                    provider_factory,
                    block_subscriber,
                    InvalidateActorRequest::LastState(s),
                    strategy_receiver,
                    confirmations,
                    max_retries,
                    max_delay,
                ))
            }
        }
    }

    async fn get_transaction_state(
        &self,
    ) -> TransactionResult<
        TransactionState,
        <PF::MiddlewareFactory as MiddlewareFactory>::Middleware,
    > {
        match self {
            // Transaction completed, return final state.
            ActorState::Finalized(s) => {
                Ok(TransactionState::Finalized(s.clone()))
            }

            // Error while sending transaction, return error.
            ActorState::TransactionErr(_, e) => TxSendErr {
                last_error: Arc::clone(&e),
            }
            .fail(),

            // Error while Invalidating transaction, return error.
            ActorState::InvalidateErr(_, e) => TxInvalidateErr {
                last_error: Arc::clone(&e),
            }
            .fail(),

            // Transaction in process, return last known state.
            ActorState::Transaction(handle) => {
                let state = TransactionState::Sending(
                    handle.state_watch.borrow().clone(),
                );
                Ok(state)
            }

            // Invalidation in process, return last known state.
            ActorState::Invalidate(handle) => {
                let state = TransactionState::Invalidating(
                    handle.state_watch.borrow().clone(),
                );
                Ok(state)
            }
        }
    }
}

enum TryActorReady<T, M: Middleware + 'static>
where
    T: Clone,
{
    Ready(FinalizedState),
    Pending(ActorHandle<T, M>),
    Error(T, Arc<WorkerError<M>>),
}

async fn try_actor_ready<T, M: Middleware>(
    mut handle: ActorHandle<T, M>,
) -> TryActorReady<T, M>
where
    T: Clone,
{
    match handle.final_state.try_recv() {
        // Actor finished with a final value.
        Ok(f) => TryActorReady::Ready(f),

        // Actor still running.
        Err(oneshot::error::TryRecvError::Empty) => {
            TryActorReady::Pending(handle)
        }

        // Actor finished running, but without a final value. This means some
        // error happened.
        Err(oneshot::error::TryRecvError::Closed) => {
            match handle.handle.await {
                // Thread finished ok, actor finished ok, but without a final
                // value. This should be unreachable.
                Ok(Ok(s)) => TryActorReady::Error(
                    s,
                    Arc::new(
                        InternalErr {
                            err: "Actor finished without final value",
                        }
                        .build(),
                    ),
                ),

                // Thread finished ok, actor finished with error.
                Ok(Err(e)) => TryActorReady::Error(
                    handle.state_watch.borrow().clone(),
                    Arc::new(e),
                ),

                // Thread finished with error.
                Err(e) => TryActorReady::Error(
                    handle.state_watch.borrow().clone(),
                    Arc::new(WorkerError::ActorJoinHandleErr { source: e }),
                ),
            }
        }
    }
}
