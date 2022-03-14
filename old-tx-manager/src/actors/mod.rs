pub(super) use actor_manager::ActorManager;

// Submodules
mod actor_manager;

mod invalidate_actor;
mod transaction_actor;

mod strategy_worker;
mod submit_worker;

// Imports
use crate::error::*;
use crate::types;

use offchain_utils::offchain_core::ethers;

use ethers::providers::Middleware;

use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;

struct ActorHandle<T, M: Middleware + 'static> {
    handle: JoinHandle<WorkerResult<T, M>>,
    final_state: oneshot::Receiver<types::FinalizedState>,
    state_watch: watch::Receiver<T>,
}
