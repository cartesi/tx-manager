use crate::error::*;
use crate::utils;

use offchain_utils::middleware_factory::MiddlewareFactory;
use offchain_utils::offchain_core::ethers;

use ethers::abi::Detokenize;
use ethers::contract::builders::ContractCall;
use ethers::providers::Middleware;
use ethers::types::{
    transaction::eip2718::TypedTransaction, Address, Bytes, NameOrAddress,
    H256, U256, U64,
};

use async_trait::async_trait;
use tokio::sync::OwnedMutexGuard;

#[derive(Clone, Debug)]
pub struct Transaction {
    pub from: Address,
    pub to: Address,
    pub value: TransferValue,
    pub call_data: Option<Bytes>,
}

#[derive(Clone, Copy, Debug)]
pub enum TransferValue {
    Value(U256),
    All,
    Nothing,
}

#[derive(Clone, Copy, Debug)]
pub struct ResubmitStrategy {
    pub gas_multiplier: Option<f64>,
    pub gas_price_multiplier: Option<f64>,
    pub rate: usize,
}

impl ResubmitStrategy {
    pub fn multiply_gas(&self, gas: U256) -> U256 {
        utils::multiply(gas, self.gas_multiplier)
    }

    pub fn multiply_gas_price(&self, price: U256) -> U256 {
        utils::multiply(price, self.gas_price_multiplier)
    }

    pub fn join(&self, other: &ResubmitStrategy) -> Self {
        let gas_multiplier = {
            let res = self
                .gas_multiplier
                .unwrap_or(1.0)
                .max(other.gas_multiplier.unwrap_or(1.0));

            if res == 1.0 {
                None
            } else {
                Some(res)
            }
        };

        let gas_price_multiplier = {
            let res = self
                .gas_price_multiplier
                .unwrap_or(1.0)
                .max(other.gas_price_multiplier.unwrap_or(1.0));

            if res == 1.0 {
                None
            } else {
                Some(res)
            }
        };

        let rate = std::cmp::min(self.rate, other.rate);

        ResubmitStrategy {
            gas_multiplier,
            gas_price_multiplier,
            rate,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TransactionSubmission {
    pub transaction: Transaction,
    pub hash: H256,
    pub nonce: U256,
    pub value: Option<U256>,
    pub gas: U256,
    pub gas_price: U256,
    pub block_submitted: U64,
}

#[derive(Clone, Debug)]
pub struct TransactionReceipt {
    pub hash: H256,
    pub index: U64,
    pub block_hash: H256,
    pub block_number: U64,
    pub gas_used: U256,
    pub cumulative_gas_used: U256,
    pub status: TransactionStatus,
}

impl std::convert::TryFrom<ethers::types::TransactionReceipt>
    for TransactionReceipt
{
    type Error = String;
    fn try_from(
        r: ethers::types::TransactionReceipt,
    ) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            hash: r.transaction_hash,

            index: r.transaction_index,

            block_hash: r.block_hash.ok_or("Transaction has no block hash")?,

            block_number: r
                .block_number
                .ok_or("Transaction has no block number")?,

            gas_used: r.gas_used.ok_or("Transaction has no gas used")?,

            cumulative_gas_used: r.cumulative_gas_used,

            status: if r.status.ok_or("Transaction has no status")? == 0.into()
            {
                TransactionStatus::Failed
            } else {
                TransactionStatus::Succeded
            },
        })
    }
}

#[derive(Clone, Debug)]
pub struct SubmisssionReceipt {
    pub submission: TransactionSubmission,
    pub receipt: TransactionReceipt,
}

#[derive(Clone, Debug)]
pub enum TransactionStatus {
    Failed,
    Succeded,
}

#[derive(Clone, Debug)]
pub enum TransactionState {
    Sending(SendState),
    Invalidating(InvalidateState),
    Finalized(FinalizedState),
}

#[derive(Clone, Debug)]
pub enum SendState {
    // Transaction is being processed.
    // Contains the transaction.
    Processing {
        transaction: Transaction,
    },

    // Transaction has been submitted.
    // Contains the submission.
    Submitted {
        submission: TransactionSubmission,
    },

    // Transaction was mined and is being confirmed.
    // Contains number of confirmations and the receipt.
    Confirming {
        confirmations: usize,
        submission_receipt: SubmisssionReceipt,
    },
}

#[derive(Clone, Debug)]
pub enum InvalidateState {
    // Original transaction still in process.
    Processing,

    // Invalidate has been requested.
    // Contains the original submission, to be invalidated.
    InvalidateRequested {
        original_submission: TransactionSubmission,
    },

    // Invalidate transaction was mined and is being confirmed.
    // Contains the number of confirmations, and original submission.
    Invalidating {
        confirmations: usize,
        original_submission: TransactionSubmission,
    },

    // Original transaction was mined and is being confirmed.
    // Contains number of confirmations and the receipt of the original.
    InvalidateFailing {
        confirmations: usize,
        submission_receipt: SubmisssionReceipt,
    },
}

#[derive(Clone, Debug)]
pub enum FinalizedState {
    Halted,
    Confirmed(TransactionReceipt),
    Invalidated,
}

impl From<SendState> for InvalidateState {
    fn from(state: SendState) -> InvalidateState {
        match state {
            SendState::Processing { transaction: _ } => {
                InvalidateState::Processing
            }

            SendState::Submitted { submission: s } => {
                InvalidateState::InvalidateRequested {
                    original_submission: s,
                }
            }

            SendState::Confirming {
                confirmations: c,
                submission_receipt: r,
            } => InvalidateState::InvalidateFailing {
                confirmations: c,
                submission_receipt: r,
            },
        }
    }
}

/// ProviderFactory is an object that encapsulates provider instantiation logic
/// and configuration. It is responsible for instantiating new providers. When
/// a recoverable provider error occurs, a new one is instantiated through the
/// factory. The StateActor receives a factory when created.
///
/// A provider is an object that implements three separate traits:
/// `StateActorProvider`, `SyncProvider` and `FoldProvider`. Each trait is used
/// by a different part of the application. The provider encapsulates web3, and
/// functions as a data access object to the blockchain.
#[async_trait]
pub trait ProviderFactory {
    type MiddlewareFactory: MiddlewareFactory;
    type Provider: TransactionProvider<
        Middleware = <Self::MiddlewareFactory as MiddlewareFactory>::Middleware,
    >;

    async fn get_provider(
        &self,
        previous: Option<Self::Provider>,
    ) -> ProviderResult<
        Self::Provider,
        <Self::MiddlewareFactory as MiddlewareFactory>::Middleware,
    >;
}

#[async_trait]
pub trait TransactionProvider: Send + Sync {
    type Middleware: Middleware;

    async fn accounts(&self) -> ProviderResult<Vec<Address>, Self::Middleware>;

    async fn send(
        &self,
        transaction: &Transaction,
        gas: U256,
        gas_price: U256,
        nonce: U256,
    ) -> ProviderResult<TransactionSubmission, Self::Middleware>;

    async fn lock_and_get_nonce(
        &self,
        address: Address,
    ) -> ProviderResult<(U256, OwnedMutexGuard<()>), Self::Middleware>;

    async fn balance(
        &self,
        address: Address,
    ) -> ProviderResult<U256, Self::Middleware>;

    async fn estimate_gas(
        &self,
        transaction: &Transaction,
    ) -> ProviderResult<U256, Self::Middleware>;

    async fn gas_price(&self) -> ProviderResult<U256, Self::Middleware>;

    async fn receipt(
        &self,
        hash: H256,
    ) -> ProviderResult<Option<TransactionReceipt>, Self::Middleware>;
}

impl std::convert::TryFrom<TypedTransaction> for Transaction {
    type Error = TxConversionError;
    fn try_from(
        tx: TypedTransaction,
    ) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            from: *tx.from().ok_or(
                TransactionIncomplete {
                    err: "Call has no `from` address",
                }
                .build(),
            )?,

            to: if let NameOrAddress::Address(a) = tx.to().ok_or(
                TransactionIncomplete {
                    err: "Call has no `to` address",
                }
                .build(),
            )? {
                *a
            } else {
                return TransactionIncomplete {
                    err: "Name not supported for `to` address".to_string(),
                }
                .fail();
            },

            value: if let Some(v) = tx.value() {
                TransferValue::Value(*v)
            } else {
                TransferValue::Nothing
            },

            call_data: tx.data().map(|data| data.clone()),
        })
    }
}

impl<M: Middleware, D: Detokenize> std::convert::TryFrom<ContractCall<M, D>>
    for Transaction
{
    type Error = TxConversionError;
    fn try_from(
        c: ContractCall<M, D>,
    ) -> std::result::Result<Self, Self::Error> {
        Transaction::try_from(c.tx)
    }
}
