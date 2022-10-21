use ethers::types::Chain;
use std::{fs::remove_file, time::Duration};

use tx_manager::{
    database::FileSystemDatabase,
    gas_oracle::DefaultGasOracle,
    manager::{Configuration, Manager},
    transaction::{Priority, Transaction, Value},
};

use utilities::{Net, TestConfiguration, TEST_CONFIGURATION_PATH};

#[tokio::test]
async fn test_mainnet_goerli() {
    test_testnet("mainnet/goerli".into(), Chain::Goerli).await;
}

#[tokio::test]
async fn test_optimist_goerli() {
    todo!()
}

#[tokio::test]
async fn test_arbitrum_goerli() {
    todo!()
}

#[tokio::test]
async fn test_polygon_goerli() {
    todo!()
}

/// Sends 5 gwei from account1 to account2.
async fn test_testnet(key: String, chain: Chain) {
    utilities::setup_tracing();

    let test_configuration = TestConfiguration::get(TEST_CONFIGURATION_PATH.into());
    let provider_http_url = test_configuration.provider_http_url.get(&key).unwrap();
    let account1 = test_configuration.account1;
    let account2 = test_configuration.account2;

    let net = Net::new(provider_http_url.clone(), chain, &account1);

    let balance1_before = net.get_balance_in_gwei(&account1).await;
    let balance2_before = net.get_balance_in_gwei(&account2).await;

    let manager = {
        const DATABASE_PATH: &str = "./test_live_database.json";
        remove_file(DATABASE_PATH).unwrap_or(());
        let manager = Manager::new(
            net.provider.clone(),
            DefaultGasOracle::new(),
            FileSystemDatabase::new(DATABASE_PATH.into()),
            net.chain,
            Configuration::default().set_block_time(Duration::from_secs(10)),
        )
        .await;
        assert!(manager.is_ok());
        manager.unwrap().0
    };

    let amount = 5;
    let transaction = Transaction {
        from: account1.clone().into(),
        to: account2.clone().into(),
        value: Value::Number(utilities::gwei_to_wei(amount).into()),
        call_data: None,
    };

    let result = manager
        .send_transaction(transaction, 2, Priority::Normal)
        .await;
    assert!(result.is_ok(), "err: {}", result.err().unwrap());

    let balance1_after = net.get_balance_in_gwei(&account1).await;
    let balance2_after = net.get_balance_in_gwei(&account2).await;
    let cost = balance2_after - balance2_before;

    assert!(balance1_after < balance1_before + amount);
    assert!(balance2_after == balance2_before + amount);
    assert!(cost > 0 && cost < 50);
}
