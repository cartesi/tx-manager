use offchain_utils::middleware_factory;
use offchain_utils::offchain_core::ethers;

use ethers::providers::Middleware;

use snafu::Snafu;
use std::sync::Arc;

///
/// Error types
///

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub")]
pub enum TxConversionError {
    #[snafu(display("Transaction incomplete: {}", err))]
    TransactionIncomplete { err: String },
}

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub")]
pub enum ReceiptConversionError {
    #[snafu(display("Receipt incomplete: {}", err))]
    ReceiptIncomplete { err: String },
}

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub")]
pub enum ProviderError<M: Middleware + 'static> {
    #[snafu(display("Transaction conversion error"))]
    ReceiptConversionErr { source: ReceiptConversionError },

    #[snafu(display("Middleware factory error: {}", source))]
    MiddlewareFactoryErr { source: middleware_factory::Error },

    #[snafu(display("Ethers provider error `{}`: {}", err, source))]
    EthersErr { source: M::Error, err: String },

    #[snafu(display("Blockchain call timed out: {}", source))]
    TimeoutErr {
        source: tokio::time::error::Elapsed,
        err: String,
    },
}

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub")]
pub enum WorkerError<M: Middleware + 'static> {
    #[snafu(display("Provider error: {}", source))]
    ProviderWorkerErr { source: ProviderError<M> },

    #[snafu(display("Call retry exceeded, last error: {}", source))]
    RetryLimitReachedErr { source: ProviderError<M> },

    #[snafu(display("Block subscriber channel dropped"))]
    SubscriberDroppedErr {},

    #[snafu(display("Actor join handle error"))]
    ActorJoinHandleErr { source: tokio::task::JoinError },

    #[snafu(display("Internal error: {}", err))]
    InternalErr { err: String },
}

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub")]
pub enum TransactionError<M: Middleware + 'static> {
    #[snafu(display("Transaction send error"))]
    TxSendErr { last_error: Arc<WorkerError<M>> },

    #[snafu(display("Transaction invalidate error"))]
    TxInvalidateErr { last_error: Arc<WorkerError<M>> },

    #[snafu(display("Transaction does not exist"))]
    TxNonexistentErr {},
}

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub")]
pub enum ManagerError<M: Middleware + 'static> {
    #[snafu(display("Provider error: {}", source))]
    ProviderErr { source: ProviderError<M> },

    #[snafu(display("Actor error"))]
    TxErr { source: TransactionError<M> },

    #[snafu(display("Transaction conversion error"))]
    TxConversionErr { source: TxConversionError },
}

///
/// Result Types
///

pub type ProviderResult<T, M> = std::result::Result<T, ProviderError<M>>;
pub type WorkerResult<T, M> = std::result::Result<T, WorkerError<M>>;
pub type TransactionResult<T, M> = std::result::Result<T, TransactionError<M>>;
pub type ManagerResult<T, M> = std::result::Result<T, ManagerError<M>>;

///
///Conversions
///

impl<M: Middleware> From<TxConversionError> for ManagerError<M> {
    fn from(err: TxConversionError) -> Self {
        ManagerError::TxConversionErr { source: err }
    }
}

impl<M: Middleware> From<ProviderError<M>> for ManagerError<M> {
    fn from(err: ProviderError<M>) -> Self {
        ManagerError::ProviderErr { source: err }
    }
}

impl<M: Middleware> From<ReceiptConversionError> for ProviderError<M> {
    fn from(err: ReceiptConversionError) -> Self {
        ProviderError::ReceiptConversionErr { source: err }
    }
}

impl<M: Middleware> From<ProviderError<M>> for WorkerError<M> {
    fn from(err: ProviderError<M>) -> Self {
        WorkerError::ProviderWorkerErr { source: err }
    }
}

impl From<std::convert::Infallible> for TxConversionError {
    fn from(i: std::convert::Infallible) -> TxConversionError {
        match i {}
    }
}
