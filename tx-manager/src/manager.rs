use core::time::Duration;

use ethers::providers::{JsonRpcClient, Middleware, Provider, ProviderError};
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::transaction::eip2930::AccessList;
use ethers::types::{
    Address, BlockId, BlockNumber, Eip1559TransactionRequest, NameOrAddress,
    TransactionReceipt, U256,
};

use crate::{db::Database, gas_pricer::GasPricer, transaction::Transaction};

#[derive(Debug)]
pub enum SendError {
    TODO,
    InconsistentDuplicates, // TODO : name
    ProviderError(ProviderError),
}

pub struct Manager<P: JsonRpcClient> {
    provider: Provider<P>,
    gas_pricer: GasPricer,
    db: Database,
}

impl<P: JsonRpcClient> Manager<P> {
    pub fn new(provider: Provider<P>, gas_pricer: GasPricer) -> Self {
        Manager {
            provider,
            gas_pricer,
            db: Database {},
        }
    }

    pub async fn send_transaction(
        &mut self,
        transaction: Transaction,
        confirmations: usize,
        interval: Option<Duration>,
    ) -> Result<TransactionReceipt, SendError> {
        // Checking for duplicate transactions.
        if let Some(receipt) = self.deduplicate(&transaction)? {
            return Ok(receipt);
        }

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
                .map_err(SendError::ProviderError)?,
        );

        println!("transaction request => {:?}", request);

        // Sending the transaction.
        let pending = self
            .provider
            .send_transaction(request, None)
            .await
            .map_err(SendError::ProviderError)?
            .confirmations(confirmations)
            .interval(interval.unwrap_or(Duration::from_secs(1)));

        // Waiting for the transaction to be confirmed.
        let receipt = pending
            .await
            .map_err(SendError::ProviderError)
            .transpose()
            .unwrap_or(Err(SendError::TODO));

        return receipt;

        // TODO : monitor pending transaction, and etc.
    }

    fn deduplicate(
        &self,
        transaction: &Transaction,
    ) -> Result<Option<TransactionReceipt>, SendError> {
        return self.db.get_transaction_receipt_for(transaction);
    }

    async fn get_nonce(&self, address: Address) -> Result<U256, SendError> {
        return self
            .provider
            .get_transaction_count(
                NameOrAddress::Address(address),
                Some(BlockId::Number(BlockNumber::Pending)),
            )
            .await
            .map_err(SendError::ProviderError);
    }
}
