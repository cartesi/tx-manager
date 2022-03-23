use core::time::Duration;

use ethers::providers::{
    JsonRpcClient, Middleware, PendingTransaction, Provider, ProviderError,
};
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::transaction::eip2930::AccessList;
use ethers::types::{
    Address, BlockId, BlockNumber, Eip1559TransactionRequest, NameOrAddress,
    TransactionReceipt, TxHash, U256, U64,
};

use tokio::time::sleep;

use crate::receipt_database::ReceiptDatabase;
use crate::{gas_pricer::GasPricer, transaction::Transaction};

// TODO => estimativa de gas => precisa multiplicar por algum valor (1.2), pra garantir que não vá da merda? (talvez).
// TODO => fazer backtrack para onde calcula o transaction gas.

#[derive(Debug)]
pub enum Error {
    TODO,
    InconsistentDuplicates, // TODO : name
    ProviderError(ProviderError),
    ReceiptDatabaseError,
}

pub struct Manager<P: JsonRpcClient, DB: ReceiptDatabase> {
    provider: Provider<P>,
    gas_pricer: GasPricer,
    db: DB,
}

impl<P: JsonRpcClient, DB: ReceiptDatabase> Manager<P, DB> {
    pub fn new(
        provider: Provider<P>,
        gas_pricer: GasPricer,
        db: DB,
    ) -> Result<Self, Error> {
        return Ok(Manager {
            provider,
            gas_pricer,
            db,
        });
    }

    pub async fn send_transaction(
        &mut self,
        transaction: Transaction,
        confirmations: usize,
        polling_time: Option<Duration>,
    ) -> Result<TransactionReceipt, Error> {
        // TODO
        let block_time = Duration::from_secs(10);
        let polling_time = polling_time.unwrap_or(Duration::from_secs(10));

        // Checking for duplicate transactions.
        if let Some(receipt) = self.deduplicate(&transaction)? {
            return Ok(receipt);
        }

        let (pending_transaction, block_number) =
            self.send(transaction).await?;
        let hash: TxHash = *pending_transaction;

        // Waiting for the transaction to be confirmed.
        loop {
            let receipt = self
                .provider
                .get_transaction_receipt(hash)
                .await
                .map_err(|_| Error::TODO)?;

            match receipt {
                Some(receipt) => {
                    let receipt_block = receipt.block_number.unwrap();
                    let current_block = self
                        .provider
                        .get_block_number()
                        .await
                        .map_err(|_| Error::TODO)?;
                    if current_block >= receipt_block + confirmations {
                        return Ok(receipt);
                    }
                }
                None => {
                    // sleep and wait
                    todo!()
                }
            }
            sleep(polling_time).await;
        }

        todo!()
    }
    /*
    let receipt_block = receipt.block_number.unwrap();
                        let current_block = todo!()
                        if current_block >= receipt_block + confirmations {
                            return Ok(receipt);
                        } else {
                            // sleep and wait
                        }*/

    fn deduplicate(
        &mut self,
        transaction: &Transaction,
    ) -> Result<Option<TransactionReceipt>, Error> {
        return self
            .db
            .get_receipt(transaction.label)
            .map_err(|_| Error::ReceiptDatabaseError);
    }

    async fn get_nonce(&self, address: Address) -> Result<U256, Error> {
        return self
            .provider
            .get_transaction_count(
                NameOrAddress::Address(address),
                Some(BlockId::Number(BlockNumber::Pending)),
            )
            .await
            .map_err(Error::ProviderError);
    }

    async fn send<'a>(
        &'a self,
        transaction: Transaction,
    ) -> Result<(PendingTransaction<'a, P>, U64), Error> {
        // Estimating gas prices.
        let (max_fee_per_gas, max_priority_fee_per_gas) =
            self.gas_pricer.estimate_eip1559_fees(transaction.priority);

        // Creating the transaction request.
        let mut request = Eip1559TransactionRequest {
            from: Some(transaction.from),
            to: Some(NameOrAddress::Address(transaction.to)),
            gas: None,
            value: Some(transaction.value.try_into()?),
            data: None,
            nonce: Some(self.get_nonce(transaction.from).await?),
            access_list: AccessList::default(),
            max_priority_fee_per_gas: Some(max_priority_fee_per_gas),
            max_fee_per_gas: Some(max_fee_per_gas),
        };

        // Estimating the transaction's gas cost.
        request.gas = Some(
            self.provider
                .estimate_gas(&TypedTransaction::Eip1559(request.clone()))
                .await
                .map_err(Error::ProviderError)?,
        );

        // TODO : log library => tracing subscriber => tokio
        println!("transaction request => {:?}", request);

        // Getting the block number from when the transaction is being sent
        let block_number = self
            .provider
            .get_block_number()
            .await
            .map_err(Error::ProviderError)?;

        // Sending the transaction.
        let pending_transaction = self
            .provider
            .send_transaction(request, None)
            .await
            .map_err(Error::ProviderError)?;

        return Ok((pending_transaction, block_number));
    }
}
