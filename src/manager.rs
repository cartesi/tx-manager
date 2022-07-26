use async_recursion::async_recursion;
use ethers::{
    providers::Middleware,
    types::{
        transaction::eip2718::TypedTransaction, Address, BlockId, BlockNumber,
        Eip1559TransactionRequest, NameOrAddress, TransactionReceipt, H256, U256, U64,
    },
    utils::keccak256,
};
use std::default::Default;
use std::time::{Duration, Instant};
use tracing::{info, trace, warn};

use crate::database::Database;
use crate::gas_oracle::{GasInfo, GasOracle};
use crate::time::{DefaultTime, Time};
use crate::transaction::{PersistentState, Priority, StaticTxData, SubmittedTxs, Transaction};

// Default values.
const TRANSACTION_MINING_TIME: Duration = Duration::from_secs(60);
const BLOCK_TIME: Duration = Duration::from_secs(20);

#[derive(Debug, thiserror::Error)]
pub enum Error<M: Middleware, GO: GasOracle, DB: Database> {
    #[error("middleware: {0}")]
    Middleware(M::Error),

    #[error("database: {0}")]
    Database(DB::Error),

    #[error("gas oracle: {0}")]
    GasOracle(GO::Error, M::Error),

    #[error("internal error: latest block is none")]
    LatestBlockIsNone,

    #[error("internal error: latest base fee is none")]
    LatestBaseFeeIsNone,
}

#[derive(Debug)]
pub struct Configuration<T: Time> {
    /// Time the transaction manager will wait to check whether a transaction
    /// was mined by a block.
    pub transaction_mining_time: Duration,

    /// Time the transaction manager will wait to check whether a block was
    /// mined.
    pub block_time: Duration,

    /// Dependency that handles process sleeping and calculating elapsed time.
    pub time: T,
}

impl Default for Configuration<DefaultTime> {
    fn default() -> Self {
        Self {
            transaction_mining_time: TRANSACTION_MINING_TIME,
            block_time: BLOCK_TIME,
            time: DefaultTime,
        }
    }
}

#[derive(Debug)]
pub struct Manager<M: Middleware, GO: GasOracle, DB: Database, T: Time> {
    provider: M,
    gas_oracle: GO,
    db: DB,
    chain_id: U64,
    configuration: Configuration<T>,
}

impl<M: Middleware, GO: GasOracle, DB: Database, T: Time> Manager<M, GO, DB, T>
where
    M: Send + Sync,
    GO: Send + Sync,
    DB: Send + Sync,
    T: Send + Sync,
{
    /// Sends and confirms any pending transaction persisted in the database
    /// before returning an instance of the transaction manager. In case a
    /// pending transaction was mined, it's receipt is also returned.
    #[tracing::instrument(level = "trace")]
    pub async fn new(
        provider: M,
        gas_oracle: GO,
        db: DB,
        chain_id: U64,
        configuration: Configuration<T>,
    ) -> Result<(Self, Option<TransactionReceipt>), Error<M, GO, DB>> {
        let mut manager = Self {
            provider,
            gas_oracle,
            db,
            chain_id,
            configuration,
        };

        let transaction_receipt = match manager.db.get_state().await.map_err(Error::Database)? {
            Some(mut state) => {
                let wait_time = manager.wait_time(state.tx_data.confirmations, None);

                let transaction_receipt = manager
                    .confirm_transaction(&mut state, None, wait_time, false)
                    .await?;

                manager.db.clear_state().await.map_err(Error::Database)?;

                Some(transaction_receipt)
            }

            None => None,
        };

        Ok((manager, transaction_receipt))
    }

    #[tracing::instrument(level = "trace")]
    pub async fn send_transaction(
        mut self,
        transaction: Transaction,
        confirmations: usize,
        priority: Priority,
    ) -> Result<(Self, TransactionReceipt), Error<M, GO, DB>> {
        let mut state = {
            let nonce = self.get_nonce(transaction.from).await?;

            let tx_data = StaticTxData {
                transaction,
                nonce,
                confirmations,
                priority,
            };

            let submitted_txs = SubmittedTxs::new();

            PersistentState {
                tx_data,
                submitted_txs,
            }
        };

        let receipt = self.send_then_confirm_transaction(&mut state, None).await?;

        info!(
            "Transaction with nonce {:?} was sent. Transaction hash = {:?}.",
            state.tx_data.nonce, receipt.transaction_hash
        );

        // Clearing information about the transaction in the database.
        self.db.clear_state().await.map_err(Error::Database)?;

        Ok((self, receipt))
    }
}

impl<M: Middleware, GO: GasOracle, DB: Database, T: Time> Manager<M, GO, DB, T>
where
    M: Send + Sync,
    GO: Send + Sync,
    DB: Send + Sync,
    T: Send + Sync,
{
    #[async_recursion]
    #[tracing::instrument(level = "trace")]
    async fn send_then_confirm_transaction(
        &mut self,
        state: &mut PersistentState,
        polling_time: Option<Duration>,
    ) -> Result<TransactionReceipt, Error<M, GO, DB>> {
        trace!("(Re)sending the transaction.");

        let tx_data = &state.tx_data;

        // Estimating gas prices and sleep times.
        let gas_info = self.gas_info(tx_data.priority).await?;
        let max_fee = gas_info.gas_price;
        let max_priority_fee = self.get_max_priority_fee(max_fee).await?;

        if let Some(block_time) = gas_info.block_time {
            self.configuration.block_time = block_time;
        }

        let wait_time = self.wait_time(tx_data.confirmations, gas_info.mining_time);

        let request = {
            // Creating the transaction request.
            let mut request: Eip1559TransactionRequest =
                tx_data.transaction.to_eip_1559_transaction_request(
                    self.chain_id,
                    tx_data.nonce,
                    max_priority_fee,
                    max_fee,
                );

            // Estimating the gas limit of the transaction.
            request.gas = Some(
                self.provider
                    .estimate_gas(&TypedTransaction::Eip1559(request.clone()))
                    .await
                    .map_err(Error::Middleware)?,
            );

            request
        };

        {
            // Calculating the transaction hash.
            let typed_transaction = &TypedTransaction::Eip1559(request.clone());
            let transaction_hash = self.transaction_hash(typed_transaction).await?;

            // Storing information about the pending
            // transaction in the database.
            state.submitted_txs.add_tx_hash(transaction_hash);
            self.db.set_state(state).await.map_err(Error::Database)?;

            trace!(
                "The manager has {:?} pending transactions.",
                state.submitted_txs.tx_count()
            );

            // Sending the transaction.
            // TODO: ignore the replacement transaction underpriced error?
            let pending_transaction = self
                .provider
                .send_transaction(request, None)
                .await
                .map_err(Error::Middleware)?;

            assert_eq!(
                transaction_hash,
                H256(*pending_transaction.as_fixed_bytes()),
                "stored hash is different from the pending transaction's hash"
            );
        };

        // Confirming the transaction.
        self.confirm_transaction(state, polling_time, wait_time, true)
            .await
    }

    #[tracing::instrument(level = "trace")]
    async fn confirm_transaction(
        &mut self,
        state: &mut PersistentState,
        polling_time: Option<Duration>,
        wait_time: Duration,
        sleep_first: bool,
    ) -> Result<TransactionReceipt, Error<M, GO, DB>> {
        trace!(
            "Confirming transaction (nonce = {:?}).",
            state.tx_data.nonce
        );

        let start_time = Instant::now();

        let polling_time = polling_time.unwrap_or(self.configuration.transaction_mining_time);

        let mut sleep_time = if sleep_first {
            polling_time
        } else {
            Duration::ZERO
        };

        loop {
            // Sleeping.
            self.configuration.time.sleep(sleep_time).await;

            // Were any of the transactions mined?
            let receipt = self.get_mined_transaction(state).await?;

            match receipt {
                Some(receipt) => {
                    let transaction_block = receipt.block_number.unwrap().as_usize();
                    let current_block = self
                        .provider
                        .get_block_number()
                        .await
                        .map_err(Error::Middleware)?
                        .as_usize();

                    trace!("Mined transaction block: {:?}.", transaction_block);
                    trace!("Current block: {:?}.", current_block);

                    // Are there enough confirmations?
                    assert!(current_block >= transaction_block);
                    let mut delta = (current_block - transaction_block) as i32;
                    delta = (state.tx_data.confirmations as i32) - delta;
                    trace!("{:?} more confirmations required.", delta);
                    if delta <= 0 {
                        return Ok(receipt);
                    }

                    sleep_time = self.configuration.block_time;
                }
                None => {
                    trace!("No transaction mined.");

                    // Have I waited too much?
                    let elapsed_time = self.configuration.time.elapsed(start_time);
                    if elapsed_time > wait_time {
                        trace!("I have waited too much!");
                        return self
                            .send_then_confirm_transaction(state, Some(polling_time))
                            .await;
                    }

                    sleep_time = polling_time;
                }
            }
        }
    }

    #[tracing::instrument(level = "trace")]
    async fn gas_info(&self, priority: Priority) -> Result<GasInfo, Error<M, GO, DB>> {
        let gas_info = self.gas_oracle.gas_info(priority).await;
        match gas_info {
            Ok(gas_info) => Ok(gas_info),

            Err(err1) => {
                warn!("Gas oracle has failed with error {:?}.", err1);
                let (max_fee, _max_priority_fee) = self
                    .provider
                    .estimate_eip1559_fees(None)
                    .await
                    .map_err(|err2| Error::GasOracle(err1, err2))?;

                Ok(GasInfo {
                    gas_price: max_fee,
                    mining_time: None,
                    block_time: None,
                })
            }
        }
    }

    #[tracing::instrument(level = "trace")]
    async fn get_mined_transaction(
        &self,
        state: &mut PersistentState,
    ) -> Result<Option<TransactionReceipt>, Error<M, GO, DB>> {
        for &hash in &state.submitted_txs {
            if let Some(receipt) = self
                .provider
                .get_transaction_receipt(hash)
                .await
                .map_err(Error::Middleware)?
            {
                return Ok(Some(receipt));
            }
        }
        Ok(None)
    }

    #[tracing::instrument(level = "trace")]
    async fn get_max_priority_fee(&self, max_fee: U256) -> Result<U256, Error<M, GO, DB>> {
        let base_fee = self
            .provider
            .get_block(BlockId::Number(BlockNumber::Latest))
            .await
            .map_err(Error::Middleware)?
            .ok_or(Error::LatestBlockIsNone)?
            .base_fee_per_gas
            .ok_or(Error::LatestBaseFeeIsNone)?;

        assert!(
            max_fee > base_fee,
            "max_fee({:?}) <= base_fee({:?})",
            max_fee,
            base_fee
        );

        Ok(max_fee - base_fee)
    }

    #[tracing::instrument(level = "trace")]
    async fn get_nonce(&self, address: Address) -> Result<U256, Error<M, GO, DB>> {
        self.provider
            .get_transaction_count(
                NameOrAddress::Address(address),
                Some(BlockId::Number(BlockNumber::Pending)),
            )
            .await
            .map_err(Error::Middleware)
    }

    /// Calculates the transaction hash.
    #[tracing::instrument(level = "trace")]
    async fn transaction_hash(
        &self,
        typed_transaction: &TypedTransaction,
    ) -> Result<H256, Error<M, GO, DB>> {
        let from = *typed_transaction.from().unwrap();
        let signature = self
            .provider
            .sign_transaction(typed_transaction, from)
            .await
            .map_err(Error::Middleware)?;
        let bytes = typed_transaction.rlp_signed(&signature);
        let hash = keccak256(bytes);
        Ok(H256(hash))
    }

    #[tracing::instrument(level = "trace")]
    fn wait_time(
        &self,
        confirmations: usize,
        transaction_mining_time: Option<Duration>,
    ) -> Duration {
        let transaction_mining_time =
            transaction_mining_time.unwrap_or(self.configuration.transaction_mining_time);

        let confirmation_time = (confirmations as u32) * self.configuration.block_time;

        transaction_mining_time + confirmation_time
    }
}
