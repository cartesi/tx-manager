use async_recursion::async_recursion;
use ethers::providers::Middleware;
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::{
    Address, BlockId, BlockNumber, Eip1559TransactionRequest, NameOrAddress,
    TransactionReceipt, TxHash, H256, U256, U64,
};
use ethers::utils::keccak256;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::info;

use crate::database::Database;
use crate::gas_oracle::{GasInfo, GasOracle};
use crate::transaction::{Priority, Transaction};

#[derive(Debug, thiserror::Error)]
pub enum ManagerError<M: Middleware> {
    #[error("manager: {0}")]
    Error(String),

    #[error("middleware: {0}")]
    Middleware(M::Error),

    #[error("set state: {0}")]
    SetState(anyhow::Error),

    #[error("get state: {0}")]
    GetState(anyhow::Error),

    #[error("clear state: {0}")]
    ClearState(anyhow::Error, TransactionReceipt),

    #[error("gas oracle: {0}")]
    GasOracle(anyhow::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct State {
    pub nonce: Option<U256>, // TODO: is this necessary?
    pub transaction: Transaction,
    pub pending_transactions: Vec<H256>, // hashes
}

#[derive(Debug)]
pub struct Manager<M: Middleware, GO: GasOracle, DB: Database> {
    provider: M,
    gas_oracle: GO,
    db: DB,
    chain_id: U64,
    mining_time: Duration,
    block_time: Duration,
}

impl<M: Middleware, GO: GasOracle, DB: Database> Manager<M, GO, DB> {
    /*
     * Sends and confirms any pending transaction persisted in the database
     * before returning an instance of the transaction manager.
     */
    pub async fn new(
        provider: M,
        gas_oracle: GO,
        db: DB,
        chain_id: U64,
    ) -> Result<(Self, Option<TransactionReceipt>), ManagerError<M>> {
        Self::new_(
            provider,
            gas_oracle,
            db,
            chain_id,
            Duration::from_secs(60),
            Duration::from_secs(20),
        )
        .await
    }

    pub async fn new_(
        provider: M,
        gas_oracle: GO,
        db: DB,
        chain_id: U64,
        mining_time: Duration,
        block_time: Duration,
    ) -> Result<(Self, Option<TransactionReceipt>), ManagerError<M>> {
        let mut manager = Manager {
            provider,
            gas_oracle,
            chain_id,
            db,
            mining_time,
            block_time,
        };
        let transaction_receipt = if let Some(mut state) = manager
            .db
            .get_state()
            .await
            .map_err(ManagerError::GetState)?
        {
            let wait_time =
                manager.wait_time(state.transaction.confirmations, None);
            let transaction_receipt = manager
                .confirm_transaction(
                    &mut state,
                    None,
                    wait_time,
                    Instant::now(),
                    false,
                )
                .await?;
            manager.db.clear_state().await.map_err(|err| {
                ManagerError::ClearState(err, transaction_receipt.clone())
            })?;
            Some(transaction_receipt)
        } else {
            None
        };

        Ok((manager, transaction_receipt))
    }

    pub async fn send_transaction(
        &mut self,
        transaction: Transaction,
        polling_time: Option<Duration>,
    ) -> Result<TransactionReceipt, ManagerError<M>> {
        let mut state = State {
            nonce: None,
            transaction,
            pending_transactions: Vec::new(),
        };

        // Storing information about the pending transaction in the database.
        self.db
            .set_state(&state)
            .await
            .map_err(ManagerError::SetState)?;

        let receipt = self.send_transaction_(&mut state, polling_time).await?;

        // Clearing information about the transaction in the database.
        self.db
            .clear_state()
            .await
            .map_err(|err| ManagerError::ClearState(err, receipt.clone()))?;

        Ok(receipt)
    }

    #[async_recursion(?Send)]
    async fn send_transaction_(
        &mut self,
        state: &mut State,
        polling_time: Option<Duration>,
    ) -> Result<TransactionReceipt, ManagerError<M>> {
        let transaction = &state.transaction;

        // Estimating gas prices and sleep times.
        let gas_info = self.gas_info(transaction.priority).await?;
        let max_fee = U256::from(gas_info.gas_price);
        let max_priority_fee = self.get_max_priority_fee(max_fee).await?;
        if let Some(block_time) = gas_info.block_time {
            self.block_time = block_time;
        }
        let wait_time = self
            .wait_time(state.transaction.confirmations, gas_info.mining_time);

        // Creating the transaction request.
        let nonce = state
            .nonce
            .unwrap_or(self.get_nonce(transaction.from).await?);
        let mut request: Eip1559TransactionRequest = transaction
            .to_eip_1559_transaction_request(nonce, max_priority_fee, max_fee);

        // Estimating the gas limit of the transaction.
        request.gas = Some(
            self.provider
                .estimate_gas(&TypedTransaction::Eip1559(request.clone()))
                .await
                .map_err(ManagerError::Middleware)?,
        );

        let start_time: Instant;
        {
            // Updating the transaction manager state.
            let typed_transaction = &TypedTransaction::Eip1559(request.clone());
            let transaction_hash =
                self.transaction_hash(&typed_transaction).await?;
            state.pending_transactions.insert(0, transaction_hash);
            self.db
                .set_state(state)
                .await
                .map_err(ManagerError::SetState)?;

            // Sending the transaction.
            let pending_transaction = self
                .provider
                .send_transaction(request, None)
                .await
                .map_err(ManagerError::Middleware)?;
            assert_eq!(transaction_hash, *pending_transaction);

            start_time = Instant::now();
        }

        // Confirming the transaction.
        self.confirm_transaction(
            state,
            polling_time,
            wait_time,
            start_time,
            true,
        )
        .await
    }

    #[tracing::instrument]
    async fn confirm_transaction(
        &mut self,
        state: &mut State,
        polling_time: Option<Duration>,
        wait_time: Duration,
        start_time: Instant,
        sleep_first: bool,
    ) -> Result<TransactionReceipt, ManagerError<M>> {
        info!("Confirming the transaction.");

        let polling_time = polling_time.unwrap_or(self.mining_time);
        let mut sleep_time = if sleep_first {
            polling_time
        } else {
            Duration::ZERO
        };

        loop {
            // Sleeping.
            sleep(sleep_time).await;

            // Were any of the transactions mined?
            let receipt = self.get_mined_transaction(state).await?;

            match receipt {
                Some(receipt) => {
                    let transaction_block =
                        receipt.block_number.unwrap().as_usize();
                    let current_block = self
                        .provider
                        .get_block_number()
                        .await
                        .map_err(ManagerError::Middleware)?
                        .as_usize();

                    // Are there enough confirmations?
                    assert!(current_block >= transaction_block);
                    let delta = (current_block - transaction_block) as u32;
                    if delta >= state.transaction.confirmations {
                        return Ok(receipt);
                    }

                    sleep_time = self.block_time;
                }
                None => {
                    // Have I waited too much?
                    if self.should_resend_transaction(start_time, wait_time) {
                        return self
                            .send_transaction_(state, Some(polling_time))
                            .await;
                    }

                    sleep_time = polling_time;
                }
            }
        }
    }

    async fn gas_info(
        &self,
        priority: Priority,
    ) -> Result<GasInfo, ManagerError<M>> {
        let gas_info = self
            .gas_oracle
            .gas_info(priority)
            .await
            .map_err(ManagerError::GasOracle)?;
        // TODO: retry and fallback to the node if the gas_oracle fails
        Ok(gas_info)
    }

    async fn get_mined_transaction(
        &self,
        state: &mut State,
    ) -> Result<Option<TransactionReceipt>, ManagerError<M>> {
        for (i, &hash) in state.pending_transactions.iter().enumerate() {
            if let Some(receipt) = self
                .provider
                .get_transaction_receipt(hash)
                .await
                .map_err(ManagerError::Middleware)?
            {
                if state.pending_transactions.len() > 1 {
                    state.pending_transactions.swap_remove(i);
                    state.pending_transactions.insert(0, hash);
                }
                return Ok(Some(receipt));
            }
        }
        Ok(None)
    }

    fn should_resend_transaction(
        &self,
        start_time: Instant,
        wait_time: Duration,
    ) -> bool {
        let elapsed_time = Duration::from_secs(start_time.elapsed().as_secs());
        elapsed_time > wait_time
    }

    async fn get_max_priority_fee(
        &self,
        max_fee: U256,
    ) -> Result<U256, ManagerError<M>> {
        let current_block = self
            .provider
            .get_block_number()
            .await
            .map_err(ManagerError::Middleware)?;
        let base_fee = self
            .provider
            .get_block(current_block)
            .await
            .map_err(ManagerError::Middleware)?
            .ok_or(ManagerError::Error("internal error 1".to_string()))?
            .base_fee_per_gas
            .ok_or(ManagerError::Error("internal error 2".to_string()))?;
        Ok(max_fee - base_fee)
    }

    async fn get_nonce(
        &self,
        address: Address,
    ) -> Result<U256, ManagerError<M>> {
        self.provider
            .get_transaction_count(
                NameOrAddress::Address(address),
                Some(BlockId::Number(BlockNumber::Pending)),
            )
            .await
            .map_err(ManagerError::Middleware)
    }

    fn wait_time(
        &self,
        confirmations: u32,
        mining_time: Option<Duration>,
    ) -> Duration {
        let mining_time = mining_time.unwrap_or(Duration::from_secs(300));
        let confirmation_time = confirmations * self.block_time;
        mining_time + confirmation_time
    }

    // Computes the transaction hash.
    async fn transaction_hash(
        &self,
        typed_transaction: &TypedTransaction,
    ) -> Result<TxHash, ManagerError<M>> {
        let from = *typed_transaction.from().unwrap();
        let signature = self
            .provider
            .sign_transaction(typed_transaction, from)
            .await
            .map_err(ManagerError::Middleware)?;
        let bytes = typed_transaction.rlp_signed(self.chain_id, &signature);
        Ok(TxHash(keccak256(bytes)))
    }
}
