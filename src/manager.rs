use async_recursion::async_recursion;
use ethers::{
    providers::Middleware,
    types::{
        transaction::eip2718::TypedTransaction, Address, BlockId, BlockNumber, Bytes,
        NameOrAddress, TransactionReceipt, H256, U256,
    },
};

use std::default::Default;
use std::fmt::Debug;
use std::time::{Duration, Instant};
use tracing::{error, info, trace, warn};

use crate::gas_oracle::{GasInfo, GasOracle, GasOracleInfo, LegacyGasInfo};
use crate::time::{DefaultTime, Time};
use crate::transaction::{PersistentState, Priority, StaticTxData, SubmittedTxs, Transaction};
use crate::{database::Database, gas_oracle::EIP1559GasInfo};

// Default values.
const TRANSACTION_MINING_TIME: Duration = Duration::from_secs(60);
const BLOCK_TIME: Duration = Duration::from_secs(20);

// ------------------------------------------------------------------------------------------------
// Error
// ------------------------------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum Error<M: Middleware, GO: GasOracle, DB: Database> {
    #[error("middleware: {0}")]
    Middleware(M::Error),

    #[error("database: {0}")]
    Database(DB::Error),

    #[error("gas oracle: error1 = ({0}), error2 = ({1})")]
    GasOracle(GO::Error, M::Error),

    #[error("nonce too low (expected: {expected_nonce}, current: {current_nonce})")]
    NonceTooLow {
        current_nonce: U256,
        expected_nonce: U256,
    },

    #[error("internal error: latest block is none")]
    LatestBlockIsNone,

    #[error("internal error: latest base fee is none")]
    LatestBaseFeeIsNone,

    #[error("internal error: incompatible gas oracle ({0})")]
    IncompatibleGasOracle(&'static str),
}

// ------------------------------------------------------------------------------------------------
// Configuration
// ------------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Configuration<T: Time> {
    /// Time it takes for a transaction to be mined by a block after being sent
    /// to the transaction pool.
    pub transaction_mining_time: Duration,

    /// Time it takes for a block to be mined. The transaction manager uses this
    /// value to calculate the polling interval when checking whether the
    /// transaction was mined.
    pub block_time: Duration,

    /// Dependency that handles process sleeping and calculating elapsed time.
    pub time: T,
}

impl<T: Time> Configuration<T> {
    pub fn set_transaction_mining_time(
        mut self,
        transaction_mining_time: Duration,
    ) -> Configuration<T> {
        self.transaction_mining_time = transaction_mining_time;
        self
    }

    pub fn set_block_time(mut self, block_time: Duration) -> Configuration<T> {
        self.block_time = block_time;
        self
    }

    pub fn set_time(mut self, time: T) -> Configuration<T> {
        self.time = time;
        self
    }
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

// ------------------------------------------------------------------------------------------------
// Chain
// ------------------------------------------------------------------------------------------------

#[derive(Copy, Clone, Debug)]
pub struct Chain {
    pub id: u64,
    pub is_legacy: bool,
}

impl Chain {
    /// For chains that implement the EIP1559.
    pub fn new(id: u64) -> Chain {
        Self {
            id,
            is_legacy: false,
        }
    }

    /// For chains that do not implement the EIP1559.
    pub fn legacy(id: u64) -> Chain {
        Self {
            id,
            is_legacy: true,
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Manager
// ------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub struct Manager<M: Middleware, GO: GasOracle, DB: Database, T: Time> {
    provider: M,
    gas_oracle: GO,
    db: DB,
    chain: Chain,
    configuration: Configuration<T>,
}

/// Public functions.
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
    #[tracing::instrument(level = "trace", skip_all)]
    pub async fn new(
        provider: M,
        gas_oracle: GO,
        db: DB,
        chain: Chain,
        configuration: Configuration<T>,
    ) -> Result<(Self, Option<TransactionReceipt>), Error<M, GO, DB>> {
        let mut manager = Self {
            provider,
            gas_oracle,
            db,
            chain,
            configuration,
        };

        trace!("Instantiating a new transaction manager => {:#?}", manager);

        let transaction_receipt = match manager.db.get_state().await.map_err(Error::Database)? {
            Some(mut state) => {
                warn!("Dealing with previous state => {:#?}", state);

                {
                    let current_nonce = manager.get_nonce(state.tx_data.transaction.from).await?;
                    let expected_nonce = state.tx_data.nonce;

                    if current_nonce > expected_nonce {
                        error!(
                            "Nonce too low! Current is `{}`, expected `{}`",
                            current_nonce, expected_nonce
                        );

                        return Err(Error::NonceTooLow {
                            current_nonce,
                            expected_nonce,
                        });
                    }
                }

                let wait_time = manager.get_wait_time(state.tx_data.confirmations, None);
                let transaction_receipt = manager
                    .confirm_transaction(&mut state, wait_time, false)
                    .await?;
                manager.db.clear_state().await.map_err(Error::Database)?;
                Some(transaction_receipt)
            }

            None => None,
        };

        Ok((manager, transaction_receipt))
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub async fn force_new(
        provider: M,
        gas_oracle: GO,
        db: DB,
        chain: Chain,
        configuration: Configuration<T>,
    ) -> Result<Self, Error<M, GO, DB>> {
        let mut manager = Self {
            provider,
            gas_oracle,
            db,
            chain,
            configuration,
        };

        trace!(
            "Forcing the instantiation of a new transaction manager => {:#?}",
            manager
        );

        trace!("Clearing DB state");
        manager.db.clear_state().await.map_err(Error::Database)?;

        Ok(manager)
    }

    /// Sends a transaction and returns the receipt.
    #[tracing::instrument(level = "trace", skip_all)]
    pub async fn send_transaction(
        mut self,
        transaction: Transaction,
        confirmations: usize,
        priority: Priority,
    ) -> Result<(Self, TransactionReceipt), Error<M, GO, DB>> {
        trace!("Sending the transaction.");

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

        let receipt = self.send_then_confirm_transaction(&mut state).await?;

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
    #[tracing::instrument(level = "trace", skip_all)]
    async fn send_then_confirm_transaction(
        &mut self,
        state: &mut PersistentState,
    ) -> Result<TransactionReceipt, Error<M, GO, DB>> {
        trace!("(Re)sending the transaction.");

        // Estimating gas prices.
        let gas_oracle_info = self.get_gas_oracle_info(state.tx_data.priority).await?;

        // Overwriting the default block time and calculating the wait time.
        if let Some(block_time) = gas_oracle_info.block_time {
            self.configuration.block_time = block_time;
        }
        let wait_time =
            self.get_wait_time(state.tx_data.confirmations, gas_oracle_info.mining_time);

        // Creating the transaction request.
        let typed_transaction: TypedTransaction = {
            let mut typed_transaction = state
                .tx_data
                .to_typed_transaction(&self.chain, gas_oracle_info.gas_info);

            // Estimating the gas limit of the transaction.
            // FIXME: "insufficient funds for transfer" is detected here!
            typed_transaction.set_gas(
                self.provider
                    .estimate_gas(&typed_transaction)
                    .await
                    .map_err(Error::Middleware)?,
            );

            typed_transaction
        };

        {
            // Calculating the transaction hash.
            let (transaction_hash, raw_transaction) =
                self.raw_transaction(&typed_transaction).await?;

            // Checking for the "already known" transactions.
            if !state.submitted_txs.contains(transaction_hash) {
                // Storing information about the pending transaction in the database.
                state.submitted_txs.add(transaction_hash);
                self.db.set_state(state).await.map_err(Error::Database)?;
            }

            // Sending the transaction.
            let result = self
                .provider
                .send_raw_transaction(raw_transaction)
                .await
                .map_err(Error::Middleware);

            match result {
                Ok(pending_transaction) => {
                    assert_eq!(
                        transaction_hash,
                        H256(*pending_transaction.as_fixed_bytes()),
                        "stored hash is different from the pending transaction's hash"
                    );
                    info!(
                        "The manager has submitted transaction with hash {:?} \
                        to the transaction pool, for a total of {:?} submitted \
                        transaction(s).",
                        transaction_hash,
                        state.submitted_txs.len()
                    );
                }
                Err(err) => {
                    if is_error(&err, "replacement transaction underpriced") {
                        assert!(!state.submitted_txs.is_empty());
                        warn!("Tried to send an underpriced transaction.");
                        /* goes back to confirm_transaction */
                    } else if is_error(&err, "already known") {
                        assert!(!state.submitted_txs.is_empty());
                        warn!("Tried to send an already known transaction.");
                        /* goes back to confirm_transaction */
                    } else {
                        error!("Error while submitting transaction: {:?}", err);
                        return Err(err);
                    }
                }
            };
        };

        // Confirming the transaction.
        self.confirm_transaction(state, wait_time, true).await
    }

    #[tracing::instrument(level = "trace", skip_all)]
    async fn confirm_transaction(
        &mut self,
        state: &mut PersistentState,
        wait_time: Duration,
        sleep_first: bool,
    ) -> Result<TransactionReceipt, Error<M, GO, DB>> {
        trace!(
            "Confirming transaction (nonce = {:?}).",
            state.tx_data.nonce
        );

        let start_time = Instant::now();
        let mut sleep_time = if sleep_first {
            self.configuration.block_time
        } else {
            Duration::ZERO
        };

        loop {
            // Sleeping.
            self.configuration.time.sleep(sleep_time).await;

            // Were any of the transactions mined?
            trace!("Were any of the transactions mined?");
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
                    trace!("{:?} more confirmation(s) required.", delta);
                    if delta <= 0 {
                        return Ok(receipt);
                    }
                }
                None => {
                    trace!("No transaction mined.");

                    // Have I waited too much?
                    let elapsed_time = self.configuration.time.elapsed(start_time);
                    if elapsed_time > wait_time {
                        trace!(
                            "I have waited too much! (elapsed = {:?}, max = {:?})",
                            elapsed_time,
                            wait_time
                        );
                        return self.send_then_confirm_transaction(state).await;
                    }
                }
            }

            sleep_time = self.configuration.block_time;
        }
    }

    /// Retrieves the gas_price (legacy) or max_fee and max_priority_fee
    /// (EIP1559) from the provider and packs it inside GasOracleInfo.
    #[tracing::instrument(level = "trace", skip_all)]
    async fn get_provider_gas_oracle_info(&self) -> Result<GasOracleInfo, M::Error> {
        let gas_info = if self.chain.is_legacy {
            trace!("Calculating legacy gas price using the provider.");
            let gas_price = self.provider.get_gas_price().await?;
            trace!("(gas_price = {:?} wei)", gas_price);
            GasInfo::Legacy(LegacyGasInfo { gas_price })
        } else {
            trace!("Estimating EIP1559 fees with the provider.");
            let (max_fee, max_priority_fee) = self.provider.estimate_eip1559_fees(None).await?;
            trace!(
                "(max_fee = {:?}, max_priority_fee = {:?})",
                max_fee,
                max_priority_fee
            );
            GasInfo::EIP1559(EIP1559GasInfo {
                max_fee,
                max_priority_fee: Some(max_priority_fee),
            })
        };
        Ok(GasOracleInfo {
            gas_info,
            mining_time: None,
            block_time: None,
        })
    }

    /// Uses the provider to calculate the max_priority_fee given the max_fee.
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

    /// Retrieves the gas_oracle_info from the gas oracle if there is one, or
    /// from the provider otherwise.
    #[tracing::instrument(level = "trace", skip_all)]
    async fn get_gas_oracle_info(
        &self,
        priority: Priority,
    ) -> Result<GasOracleInfo, Error<M, GO, DB>> {
        match self.gas_oracle.get_info(priority).await {
            Ok(mut gas_oracle_info) => {
                assert_eq!(gas_oracle_info.gas_info.is_legacy(), self.chain.is_legacy);

                if let GasInfo::EIP1559(mut eip1559_gas_info) = gas_oracle_info.gas_info {
                    if eip1559_gas_info.max_priority_fee.is_none() {
                        eip1559_gas_info.max_priority_fee =
                            Some(self.get_max_priority_fee(eip1559_gas_info.max_fee).await?);
                        gas_oracle_info.gas_info = GasInfo::EIP1559(eip1559_gas_info);
                    };
                }

                Ok(gas_oracle_info)
            }
            Err(err1) => {
                warn!(
                    "Gas oracle has failed and/or is defaulting to the provider ({}).",
                    err1.to_string()
                );
                self.get_provider_gas_oracle_info()
                    .await
                    .map_err(|err2| Error::GasOracle(err1, err2))
            }
        }
    }

    #[tracing::instrument(level = "trace", skip_all)]
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

    #[tracing::instrument(level = "trace", skip_all)]
    async fn get_nonce(&self, address: Address) -> Result<U256, Error<M, GO, DB>> {
        self.provider
            .get_transaction_count(
                NameOrAddress::Address(address),
                Some(BlockId::Number(BlockNumber::Pending)),
            )
            .await
            .map_err(Error::Middleware)
    }

    /// Returns the transaction hash and the raw transaction.
    #[tracing::instrument(level = "trace", skip_all)]
    async fn raw_transaction(
        &self,
        typed_transaction: &TypedTransaction,
    ) -> Result<(H256, Bytes), Error<M, GO, DB>> {
        let from = *typed_transaction.from().unwrap();
        let signature = self
            .provider
            .sign_transaction(typed_transaction, from)
            .await
            .map_err(Error::Middleware)?;
        let hash = typed_transaction.hash(&signature);
        let rlp_data = typed_transaction.rlp_signed(&signature);
        Ok((hash, rlp_data))
    }

    /// TODO: docs.
    #[tracing::instrument(level = "trace", skip_all)]
    fn get_wait_time(
        &self,
        confirmations: usize,
        transaction_mining_time: Option<Duration>,
    ) -> Duration {
        let transaction_mining_time =
            transaction_mining_time.unwrap_or(self.configuration.transaction_mining_time);
        let confirmation_time = if confirmations > 0 {
            confirmations as u32
        } else {
            1
        } * self.configuration.block_time;
        transaction_mining_time + confirmation_time
    }
}

fn is_error<E>(err: &E, s: &str) -> bool
where
    E: Debug,
{
    format!("{:?}", err).contains(s)
}
