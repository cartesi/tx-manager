use async_recursion::async_recursion;
use core::time::Duration;
use serde::{Deserialize, Serialize};

use ethers::providers::{JsonRpcClient, Middleware, Provider};
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::transaction::eip2930::AccessList;
use ethers::types::{
    Address, BlockId, BlockNumber, Eip1559TransactionRequest, NameOrAddress,
    TransactionReceipt, H256, U256,
};

use tokio::time::sleep;

use crate::database::Database;
use crate::{gas_pricer::GasPricer, transaction::Transaction};

#[derive(Debug)]
pub enum Error<DatabaseError> {
    TODO,
    CouldNotSetTransaction(DatabaseError),
    CouldNotUpdateTransactionState(DatabaseError),
    CouldNotGetTransactionState(DatabaseError),
    CouldNotClearTransaction(DatabaseError, TransactionReceipt),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct State {
    nonce: Option<U256>,
    transaction: Transaction,
    pending_transactions: Vec<H256>, // hashes
}

pub struct Manager<P: JsonRpcClient, DB: Database> {
    provider: Provider<P>,
    gas_pricer: GasPricer,
    db: DB,
    polling_time: Duration,
    block_time: Duration,
}

impl<P: JsonRpcClient, DB: Database> Manager<P, DB> {
    /*
     * Sends and confirms any pending transaction persisted in the database
     * before returning an instance of the transaction manager.
     */
    pub async fn new(
        provider: Provider<P>,
        gas_pricer: GasPricer,
        db: DB,
    ) -> Result<Self, Error<DB::Error>> {
        let manager = Manager {
            provider,
            gas_pricer,
            db,
            polling_time: Duration::from_secs(60),
            block_time: Duration::from_secs(10),
        };

        // Checking for pending transactions.
        if let Some(mut state) = manager
            .db
            .get_state()
            .await
            .map_err(Error::CouldNotGetTransactionState)?
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
    ) -> Result<TransactionReceipt, Error<DB::Error>> {
        let mut state = State {
            nonce: None,
            transaction,
            pending_transactions: Vec::new(),
        };

        // Storing information about the pending transaction in the database.
        self.db
            .set_state(&state)
            .await
            .map_err(Error::CouldNotSetTransaction)?;

        let receipt = self
            .submit_transaction(
                &mut state,
                polling_time.unwrap_or(self.polling_time),
            )
            .await?;

        // Clearing information about the transaction in the database.
        self.db.clear_state().await.map_err(|err| {
            Error::CouldNotClearTransaction(err, receipt.clone())
        })?;

        return Ok(receipt);
    }

    #[async_recursion(?Send)]
    async fn submit_transaction(
        &self,
        state: &mut State,
        polling_time: Duration,
    ) -> Result<TransactionReceipt, Error<DB::Error>> {
        let transaction = &state.transaction;

        // Estimating gas prices.
        let (max_fee_per_gas, max_priority_fee_per_gas) = self
            .gas_pricer
            .estimate_eip1559_fees(transaction.priority)
            .await;

        // Creating the transaction request.
        let mut request = Eip1559TransactionRequest {
            from: Some(transaction.from),
            to: Some(NameOrAddress::Address(transaction.to)),
            gas: None,
            value: Some(transaction.value.try_into().map_err(|_| Error::TODO)?),
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
                .map_err(|_| Error::TODO)?,
        );

        // Sending the transaction.
        let pending_transaction = self
            .provider
            .send_transaction(request, None)
            .await
            .map_err(|_| Error::TODO)?;

        // Updating the transaction manager state.
        state.pending_transactions.insert(0, *pending_transaction);
        self.db
            .set_state(state)
            .await
            .map_err(Error::CouldNotUpdateTransactionState)?;

        // Confirming the transaction
        return self.confirm_transaction(state, polling_time, true).await;
    }

    async fn confirm_transaction(
        &self,
        state: &mut State,
        polling_time: Duration,
        sleep_first: bool,
    ) -> Result<TransactionReceipt, Error<DB::Error>> {
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
                .map_err(|_| Error::TODO)?;

            match receipt {
                Some(receipt) => {
                    let transaction_block =
                        receipt.block_number.unwrap().as_usize();
                    let current_block = self
                        .provider
                        .get_block_number()
                        .await
                        .map_err(|_| Error::TODO)?
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
    ) -> Result<Option<TransactionReceipt>, Error<DB::Error>> {
        for (i, &hash) in state.pending_transactions.iter().enumerate() {
            if let Some(receipt) = self
                .provider
                .get_transaction_receipt(hash)
                .await
                .map_err(|_| Error::TODO)?
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

    async fn get_nonce(
        &self,
        address: Address,
    ) -> Result<U256, Error<DB::Error>> {
        return self
            .provider
            .get_transaction_count(
                NameOrAddress::Address(address),
                Some(BlockId::Number(BlockNumber::Pending)),
            )
            .await
            .map_err(|_| Error::TODO);
    }
}
