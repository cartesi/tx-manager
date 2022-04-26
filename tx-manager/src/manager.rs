use anyhow::Error;
use async_recursion::async_recursion;
use core::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

use ethers::providers::{JsonRpcClient, Middleware, Provider};
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::transaction::eip2930::AccessList;
use ethers::types::{
    Address, BlockId, BlockNumber, Eip1559TransactionRequest, NameOrAddress,
    TransactionReceipt, H256, U256,
};

use crate::database::Database;
use crate::gas_oracle::{GasInfo, GasOracle};
use crate::transaction::{Priority, Transaction};

#[derive(Debug, thiserror::Error)]
pub enum ManagerError {
    #[error("todo remove")]
    TODO,

    #[error("set state: {0}")]
    SetState(Error),

    #[error("get state: {0}")]
    GetState(Error),

    #[error("clear state: {0}")]
    ClearState(Error, TransactionReceipt),

    #[error("gas oracle: {0}")]
    GasOracle(Error),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct State {
    pub nonce: Option<U256>,
    pub transaction: Transaction,
    pub pending_transactions: Vec<H256>, // hashes
}

pub struct Manager<P: JsonRpcClient, Oracle: GasOracle, DB: Database> {
    provider: Provider<P>,
    gas_oracle: Oracle,
    db: DB,
    polling_time: Duration,
    block_time: Duration,
}

impl<P: JsonRpcClient, GO: GasOracle, DB: Database> Manager<P, GO, DB> {
    /*
     * Sends and confirms any pending transaction persisted in the database
     * before returning an instance of the transaction manager.
     */
    pub async fn new(
        provider: Provider<P>,
        gas_oracle: GO,
        db: DB,
    ) -> Result<Self, ManagerError> {
        let manager = Manager {
            provider,
            gas_oracle,
            db,
            polling_time: Duration::from_secs(60),
            block_time: Duration::from_secs(10),
        };

        // Checking for pending transactions.
        if let Some(mut state) = manager
            .db
            .get_state()
            .await
            .map_err(ManagerError::GetState)?
        {
            manager
                .confirm_transaction(&mut state, manager.polling_time, false)
                .await?;
        }

        return Ok(manager);
    }

    pub async fn send_transaction(
        &mut self,
        transaction: Transaction,
        polling_time: Option<Duration>,
    ) -> Result<TransactionReceipt, ManagerError> {
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

        let receipt = self
            .submit_transaction(
                &mut state,
                polling_time.unwrap_or(self.polling_time),
            )
            .await?;

        // Clearing information about the transaction in the database.
        self.db
            .clear_state()
            .await
            .map_err(|err| ManagerError::ClearState(err, receipt.clone()))?;

        return Ok(receipt);
    }

    async fn gas_info(
        &self,
        priority: Priority,
    ) -> Result<GasInfo, ManagerError> {
        let gas_info = self
            .gas_oracle
            .gas_info(priority)
            .await
            .map_err(ManagerError::GasOracle)?;
        // TODO: retry and fallback to the node if the gas_oracle fails
        return Ok(gas_info);
    }

    #[async_recursion(?Send)]
    async fn submit_transaction(
        &self,
        state: &mut State,
        polling_time: Duration,
    ) -> Result<TransactionReceipt, ManagerError> {
        let transaction = &state.transaction;

        // Estimating gas prices.
        let gas_info = self.gas_info(transaction.priority).await?;
        let max_fee_per_gas = U256::from(gas_info.gas_price);
        let max_priority_fee_per_gas = self.get_max_priority_fee_per_gas();

        // Creating the transaction request.
        let mut request = Eip1559TransactionRequest {
            from: Some(transaction.from),
            to: Some(NameOrAddress::Address(transaction.to)),
            gas: None,
            value: Some(
                transaction
                    .value
                    .try_into()
                    .map_err(|_| ManagerError::TODO)?,
            ),
            data: None,
            nonce: Some(
                state
                    .nonce
                    .unwrap_or(self.get_nonce(transaction.from).await?),
            ),
            access_list: AccessList::default(),
            max_priority_fee_per_gas: Some(max_priority_fee_per_gas),
            max_fee_per_gas: Some(max_fee_per_gas),
        };

        // Estimating the gas limit of the transaction.
        request.gas = Some(
            self.provider
                .estimate_gas(&TypedTransaction::Eip1559(request.clone()))
                .await
                .map_err(|_| ManagerError::TODO)?,
        );

        // Sending the transaction.
        let pending_transaction = self
            .provider
            .send_transaction(request, None)
            .await
            .map_err(|_| ManagerError::TODO)?;

        // Updating the transaction manager state.
        state.pending_transactions.insert(0, *pending_transaction);
        self.db
            .set_state(state)
            .await
            .map_err(ManagerError::SetState)?;

        // Confirming the transaction
        return self.confirm_transaction(state, polling_time, true).await;
    }

    async fn confirm_transaction(
        &self,
        state: &mut State,
        polling_time: Duration,
        sleep_first: bool,
    ) -> Result<TransactionReceipt, ManagerError> {
        let mut sleep_time = if sleep_first {
            polling_time
        } else {
            Duration::from_secs(0)
        };

        loop {
            // Sleeping.
            sleep(sleep_time).await;

            // Were any of the transactions mined?
            let receipt = self
                .get_mined_transaction(state)
                .await
                .map_err(|_| ManagerError::TODO)?;

            match receipt {
                Some(receipt) => {
                    let transaction_block =
                        receipt.block_number.unwrap().as_usize();
                    let current_block = self
                        .provider
                        .get_block_number()
                        .await
                        .map_err(|_| ManagerError::TODO)?
                        .as_usize();

                    // Are there enough confirmations?
                    if current_block - transaction_block
                        >= state.transaction.confirmations
                    {
                        return Ok(receipt);
                    }

                    sleep_time = self.block_time;
                }
                None => {
                    // Have I waited too much?
                    if self.should_resend_transaction() {
                        return self
                            .submit_transaction(state, polling_time)
                            .await;
                    }

                    sleep_time = polling_time;
                }
            }
        }
    }

    async fn get_mined_transaction(
        &self,
        state: &mut State,
    ) -> Result<Option<TransactionReceipt>, ManagerError> {
        for (i, &hash) in state.pending_transactions.iter().enumerate() {
            if let Some(receipt) = self
                .provider
                .get_transaction_receipt(hash)
                .await
                .map_err(|_| ManagerError::TODO)?
            {
                if state.pending_transactions.len() > 1 {
                    state.pending_transactions.swap_remove(i);
                    state.pending_transactions.insert(0, hash);
                }
                return Ok(Some(receipt));
            }
        }
        return Ok(None);
    }

    fn should_resend_transaction(&self) -> bool {
        // check for the average mining time
        unimplemented!()
    }

    fn get_max_priority_fee_per_gas(&self) -> U256 {
        todo!();
    }

    async fn get_nonce(&self, address: Address) -> Result<U256, ManagerError> {
        return self
            .provider
            .get_transaction_count(
                NameOrAddress::Address(address),
                Some(BlockId::Number(BlockNumber::Pending)),
            )
            .await
            .map_err(|_| ManagerError::TODO);
    }
}
