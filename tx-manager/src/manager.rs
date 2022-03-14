use ethers::providers::{JsonRpcClient, Middleware, Provider, ProviderError};
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::transaction::eip2930::AccessList;
use ethers::types::{
    Address, BlockId, BlockNumber, Eip1559TransactionRequest, NameOrAddress,
    TransactionReceipt, H160, U256,
};

use crate::{db::Database, transaction::Transaction};

pub struct GasPricer {}

impl GasPricer {
    fn estimate_gas_price(&self, transaction: &mut Eip1559TransactionRequest) {
        // TODO
    }
}

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
    ) -> Result<TransactionReceipt, SendError> {
        if let Some(receipt) = self.deduplicate(&transaction)? {
            return Ok(receipt);
        }

        let address = H160::zero();
        let nonce = self.get_nonce(address).await?;

        let mut request = Eip1559TransactionRequest {
            from: Some(transaction.from),
            to: Some(NameOrAddress::Address(transaction.to)),
            gas: None,
            value: Some(transaction.value.get()),
            data: None,
            nonce: Some(nonce),
            access_list: AccessList::default(),
            max_priority_fee_per_gas: None,
            max_fee_per_gas: None,
        };

        let typed_transaction = TypedTransaction::Eip1559(request.clone());

        request.gas = Some(
            self.provider
                .estimate_gas(&typed_transaction)
                .await
                .map_err(SendError::ProviderError)?,
        );

        self.gas_pricer.estimate_gas_price(&mut request);

        let pending_transaction = self
            .provider
            .send_transaction(request, None)
            .await
            .map_err(SendError::ProviderError)?;

        pending_transaction.confirmations(confirmations);

        // TODO : monitor pending transaction, and etc.

        return Err(SendError::TODO);
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
