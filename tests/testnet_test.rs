use async_trait::async_trait;
use ethers::{
    prelude::{k256::ecdsa::SigningKey, Http, Provider, SignerMiddleware, Wallet},
    providers::Middleware,
    types::TransactionReceipt,
};
use std::{fs::remove_file, time::Duration};

use eth_tx_manager::{
    database::{Database, FileSystemDatabase},
    gas_oracle::{GasInfo, GasOracle, GasOracleInfo, LegacyGasInfo},
    manager::{Configuration, Manager},
    time::Time,
    transaction::{Priority, Transaction, Value},
    Chain,
};

use utilities::{Account, ProviderWrapper, TestConfiguration, TEST_CONFIGURATION_PATH};

// IMPORTANT: If one or more of the github tests fail because there are no
// funds, send ethers to 0x8bc95ac74684e81054f7cf0ac424aa6b2de3879b in the
// appropriate chain.

#[tokio::test]
async fn test_ethereum() {
    test_testnet_ok("ethereum", Chain::new(5)).await;
    test_testnet_ok("ethereum", Chain::legacy(5)).await;
}

#[tokio::test]
async fn test_polygon() {
    test_testnet_ok("polygon", Chain::new(80001)).await;
}

#[tokio::test]
async fn test_optimism() {
    test_testnet_ok("optimism", Chain::new(420)).await;
    test_testnet_ok("optimism", Chain::legacy(420)).await;
}

/// We skip this test because we don't get much ether from the Arbitrum faucet.
#[tokio::test]
#[ignore]
async fn test_arbitrum() {
    test_testnet_ok("arbitrum", Chain::new(421613)).await;
    test_testnet_ok("arbitrum", Chain::legacy(421613)).await;
}

// ------------------------------------------------------------------------------------------------
// Auxiliary
// ------------------------------------------------------------------------------------------------

const AMOUNT: u64 = 5;

/// Sends 5 gwei from account1 to account2.
async fn test_testnet_ok(key: &str, chain: Chain) {
    utilities::setup_tracing();

    let test_configuration = TestConfiguration::get(TEST_CONFIGURATION_PATH.into());
    let provider_http_url = test_configuration.provider_http_url.get(key).unwrap();
    let account1 = test_configuration.account1;
    let account2 = test_configuration.account2;

    let provider = ProviderWrapper::new(provider_http_url.clone(), chain, &account1);
    let manager = create_manager(key, chain, provider.clone()).await;

    let balance = provider
        .get_balance(account1.clone(), account2.clone())
        .await;

    let result = send_transaction(manager, account1, account2).await;
    assert!(result.is_ok(), "err: {}", result.err().unwrap());

    provider.check_transaction_balance(balance, AMOUNT).await;
}

/*
/// Expected to fail with the "EIP-1559 not activated" error.
async fn test_eip1559_fail(key: &str, chain: Chain) {
    utilities::setup_tracing();

    let test_configuration = TestConfiguration::get(TEST_CONFIGURATION_PATH.into());
    let provider_http_url = test_configuration.provider_http_url.get(key).unwrap();
    let account1 = test_configuration.account1;
    let account2 = test_configuration.account2;

    let provider = ProviderWrapper::new(provider_http_url.clone(), chain, &account1);
    let manager = create_manager(key, chain, provider.clone()).await;

    provider
        .get_balance(account1.clone(), account2.clone())
        .await;

    let result = send_transaction(manager, account1, account2).await;
    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(
        format!("{:?}", err).contains("EIP-1559 not activated"),
        "{:?}",
        err
    );
}
*/

async fn create_manager(
    key: &str,
    chain: Chain,
    provider: ProviderWrapper,
) -> Manager<
    SignerMiddleware<Provider<Http>, Wallet<SigningKey>>,
    TestnetGasOracle,
    FileSystemDatabase,
    eth_tx_manager::time::DefaultTime,
> {
    let database_path = format!("{}_test_database.json", key,);
    remove_file(database_path.clone()).unwrap_or(());
    let manager = Manager::new(
        provider.inner.clone(),
        TestnetGasOracle {
            provider: provider.clone(),
            is_legacy: chain.is_legacy,
        },
        FileSystemDatabase::new(database_path),
        chain,
        Configuration::default().set_block_time(Duration::from_secs(10)),
    )
    .await;
    assert!(manager.is_ok());
    manager.unwrap().0
}

async fn send_transaction<M: Middleware, GO: GasOracle, DB: Database, T: Time>(
    manager: Manager<M, GO, DB, T>,
    from: Account,
    to: Account,
) -> Result<(Manager<M, GO, DB, T>, TransactionReceipt), eth_tx_manager::Error<M, GO, DB>>
where
    M: Send + Sync,
    GO: Send + Sync,
    DB: Send + Sync,
    T: Send + Sync,
{
    let transaction = Transaction {
        from: from.into(),
        to: to.into(),
        value: Value::Number(utilities::gwei_to_wei(AMOUNT)),
        call_data: None,
    };
    manager
        .send_transaction(transaction, 3, Priority::Normal)
        .await
}

// ------------------------------------------------------------------------------------------------
// TestnetGasOracle
// ------------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct TestnetGasOracle {
    provider: ProviderWrapper,
    is_legacy: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum TestnetGasOracleError {
    #[error("defaulting")]
    Default,
}

#[async_trait]
impl GasOracle for TestnetGasOracle {
    type Error = TestnetGasOracleError;

    async fn get_info(&self, _: Priority) -> Result<GasOracleInfo, Self::Error> {
        if self.is_legacy {
            // The provider's gas oracle returns the base fee of the latest block. We
            // multiply it by two to avoid "max fee per gas less than block base
            // fee" errors.
            let gas_price = self.provider.inner.get_gas_price().await.unwrap();
            let gas_price = gas_price.checked_mul(2.into()).unwrap();
            Ok(GasOracleInfo {
                gas_info: GasInfo::Legacy(LegacyGasInfo { gas_price }),
                mining_time: None,
                block_time: None,
            })
        } else {
            Err(TestnetGasOracleError::Default)
        }
    }
}
