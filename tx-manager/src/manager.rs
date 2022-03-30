use async_recursion::async_recursion;
use core::time::Duration;

use ethers::providers::{JsonRpcClient, Middleware, Provider};
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::transaction::eip2930::AccessList;
use ethers::types::{
    Address, BlockId, BlockNumber, Eip1559TransactionRequest, NameOrAddress,
    TransactionReceipt, U256,
};

use tokio::time::sleep;

use crate::database::Database;
use crate::{gas_pricer::GasPricer, transaction::Transaction};

#[derive(Debug)]
pub enum Error<DatabaseError> {
    TODO,
    CouldNotStoreTransaction(DatabaseError),
    CouldNotUpdateTransactionState(DatabaseError),
    CouldNotClearTransaction(DatabaseError, TransactionReceipt),
}

pub struct Manager<P: JsonRpcClient, DB: Database> {
    provider: Provider<P>,
    gas_pricer: GasPricer,
    db: DB,
    polling_time: Duration,
    block_time: Duration,
}

impl<P: JsonRpcClient, DB: Database> Manager<P, DB> {
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

        if let Ok(transaction) = manager.has_pending_transaction() {
            manager.confirm_transaction(&transaction, None).await?;
        }

        return Ok(manager);
    }

    pub fn has_pending_transaction(
        &self,
    ) -> Result<Transaction, Error<DB::Error>> {
        unimplemented!()
    }

    pub async fn send_transaction(
        &mut self,
        transaction: Transaction,
        polling_time: Option<Duration>,
    ) -> Result<TransactionReceipt, Error<DB::Error>> {
        // Storing information about the pending transaction in the database.
        self.db
            .store_transaction(&transaction)
            .await
            .map_err(Error::CouldNotStoreTransaction)?;

        let receipt =
            self.submit_transaction(&transaction, polling_time).await?;

        // Clearing information about the transaction in the database.
        self.db.clear_transaction().await.map_err(|err| {
            Error::CouldNotClearTransaction(err, receipt.clone())
        })?;

        return Ok(receipt);
    }

    #[async_recursion(?Send)]
    async fn submit_transaction(
        &self,
        transaction: &Transaction,
        polling_time: Option<Duration>,
    ) -> Result<TransactionReceipt, Error<DB::Error>> {
        // Estimating gas prices.
        let (max_fee_per_gas, max_priority_fee_per_gas) =
            self.gas_pricer.estimate_eip1559_fees(transaction.priority);

        // Creating the transaction request.
        let mut request = Eip1559TransactionRequest {
            from: Some(transaction.from),
            to: Some(NameOrAddress::Address(transaction.to)),
            gas: None,
            value: Some(transaction.value.try_into().map_err(|_| Error::TODO)?),
            data: None,
            nonce: Some(self.get_nonce(transaction.from).await?),
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

        // Updating the transaction manager state.
        self.db
            .update_transaction(request.clone())
            .await
            .map_err(Error::CouldNotUpdateTransactionState)?;

        // Sending the transaction.
        self.provider
            .send_transaction(request, None)
            .await
            .map_err(|_| Error::TODO)?;

        // Confirming the transaction
        return self.confirm_transaction(&transaction, polling_time).await;
    }

    async fn confirm_transaction(
        &self,
        transaction: &Transaction,
        optional_polling_time: Option<Duration>,
    ) -> Result<TransactionReceipt, Error<DB::Error>> {
        let polling_time = optional_polling_time.unwrap_or(self.polling_time);
        let mut sleep_time = polling_time;

        loop {
            // Sleeping for the average block time.
            sleep(sleep_time).await;

            // Were any of the transactions mined?
            match self.get_mined_transaction().await {
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
                        >= transaction.confirmations
                    {
                        return Ok(receipt);
                    }

                    sleep_time = self.block_time;
                }
                None => {
                    // Have I waited too much?
                    if self.should_resend_transaction() {
                        return self
                            .submit_transaction(
                                &transaction,
                                optional_polling_time,
                            )
                            .await;
                    }

                    sleep_time = polling_time;
                }
            }
        }
    }

    async fn get_mined_transaction(&self) -> Option<TransactionReceipt> {
        /*
        let receipt = self
            .provider
            .get_transaction_receipt(*pending_transaction)
            .await
            .map_err(|_| Error::TODO)?;
        */
        unimplemented!()
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
