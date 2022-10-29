use async_trait::async_trait;
use ethers::{
    prelude::{k256::ecdsa::SigningKey, Http, Provider, SignerMiddleware, Wallet},
    providers::Middleware,
    types::TransactionReceipt,
};
use serial_test::serial;
use std::{fs::remove_file, time::Duration};

use tx_manager::{
    database::{Database, FileSystemDatabase},
    gas_oracle::{GasInfo, GasOracle, GasOracleInfo, LegacyGasInfo},
    manager::{Configuration, Manager},
    time::Time,
    transaction::{Priority, Transaction, Value},
    Chain,
};

use utilities::{Account, ProviderWrapper, TestConfiguration, TEST_CONFIGURATION_PATH};

#[tokio::test]
#[serial]
async fn test_ethereum_legacy() {
    test_testnet_ok(
        "ethereum",
        "ok legacy",
        Chain {
            id: 5,
            is_legacy: true,
        },
    )
    .await;
}

#[tokio::test]
#[serial]
async fn test_ethereum_eip1559() {
    test_testnet_ok(
        "ethereum",
        "ok EIP1559",
        Chain {
            id: 5,
            is_legacy: false,
        },
    )
    .await;
}

#[tokio::test]
async fn test_polygon() {
    test_testnet_ok(
        "polygon",
        "ok EIP1559",
        Chain {
            id: 80001,
            is_legacy: false,
        },
    )
    .await;
}

#[tokio::test]
async fn test_optimism_legacy() {
    test_testnet_ok(
        "optimism",
        "ok EIP1559",
        Chain {
            id: 420,
            is_legacy: true,
        },
    )
    .await;
}

#[tokio::test]
async fn test_optimism_eip1559_fail() {
    test_eip1559_fail(
        "optimism",
        "fail EIP1559",
        Chain {
            id: 420,
            is_legacy: false,
        },
    )
    .await;
}

/// We skip this test because we don't get much ether from the Arbitrum faucet.
#[tokio::test]
#[ignore]
async fn test_arbitrum_eip1559_fail() {
    test_eip1559_fail(
        "arbitrum",
        "fail EIP1559",
        Chain {
            id: 421613,
            is_legacy: true,
        },
    )
    .await;
}

/// We skip this test because we don't get much ether from the Arbitrum faucet.
#[tokio::test]
#[ignore]
async fn test_arbitrum_legacy() {
    test_testnet_ok(
        "arbitrum",
        "ok legacy",
        Chain {
            id: 421613,
            is_legacy: true,
        },
    )
    .await;
}

// ------------------------------------------------------------------------------------------------
// Auxiliary
// ------------------------------------------------------------------------------------------------

const AMOUNT: u64 = 5;

/// Sends 5 gwei from account1 to account2.
async fn test_testnet_ok(key: &str, description: &str, chain: Chain) {
    utilities::setup_tracing();

    let test_configuration = TestConfiguration::get(TEST_CONFIGURATION_PATH.into());
    let provider_http_url = test_configuration.provider_http_url.get(key).unwrap();
    let account1 = test_configuration.account1;
    let account2 = test_configuration.account2;

    let provider = ProviderWrapper::new(provider_http_url.clone(), chain, &account1);
    let manager = create_manager(key, description, chain, provider.clone()).await;

    let balance = provider
        .get_balance(account1.clone(), account2.clone())
        .await;

    let result = send_transaction(manager, account1, account2).await;
    assert!(result.is_ok(), "err: {}", result.err().unwrap());

    provider.check_transaction_balance(balance, AMOUNT).await;
}

/// Expected to fail with the "EIP-1559 not activated" error.
async fn test_eip1559_fail(key: &str, description: &str, chain: Chain) {
    utilities::setup_tracing();

    let test_configuration = TestConfiguration::get(TEST_CONFIGURATION_PATH.into());
    let provider_http_url = test_configuration.provider_http_url.get(key).unwrap();
    let account1 = test_configuration.account1;
    let account2 = test_configuration.account2;

    let provider = ProviderWrapper::new(provider_http_url.clone(), chain, &account1);
    let manager = create_manager(key, description, chain, provider.clone()).await;

    let result = send_transaction(manager, account1, account2).await;
    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(
        format!("{:?}", err).contains("EIP-1559 not activated"),
        "{:?}",
        err
    );
}

async fn create_manager(
    key: &str,
    description: &str,
    chain: Chain,
    provider: ProviderWrapper,
) -> Manager<
    SignerMiddleware<Provider<Http>, Wallet<SigningKey>>,
    TestnetGasOracle,
    FileSystemDatabase,
    tx_manager::time::DefaultTime,
> {
    let database_path = format!(
        "{}_{}_test_database.json",
        key,
        description.replace(" ", "_")
    );
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
) -> Result<(Manager<M, GO, DB, T>, TransactionReceipt), tx_manager::Error<M, GO, DB>>
where
    M: Send + Sync,
    GO: Send + Sync,
    DB: Send + Sync,
    T: Send + Sync,
{
    let transaction = Transaction {
        from: from.into(),
        to: to.into(),
        value: Value::Number(utilities::gwei_to_wei(AMOUNT).into()),
        call_data: None,
    };
    manager
        .send_transaction(transaction, 0, Priority::Normal)
        .await
}

// ------------------------------------------------------------------------------------------------
// TestnetGasOracle
// ------------------------------------------------------------------------------------------------

#[derive(Debug)]
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
            // The provider's gas oracle simply returns the base fee of the latest block. We
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
