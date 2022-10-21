use std::{fs::remove_file, time::Duration};

use tx_manager::{
    database::FileSystemDatabase,
    gas_oracle::DefaultGasOracle,
    manager::{Configuration, Manager},
    transaction::{Priority, Transaction, Value},
    Chain,
};

use utilities::{Net, TestConfiguration, TEST_CONFIGURATION_PATH};

#[tokio::test]
async fn test_ethereum() {
    let chain = Chain {
        id: 5,
        is_legacy: false,
    };
    test_testnet("ethereum".into(), chain).await;
}

#[tokio::test]
async fn test_polygon() {
    let chain = Chain {
        id: 80001,
        is_legacy: false,
    };
    test_testnet("polygon".into(), chain).await;
}

#[tokio::test]
async fn test_optimism() {
    let chain = Chain {
        id: 420,
        is_legacy: true,
    };
    test_testnet("optimism".into(), chain).await;
}

#[tokio::test]
async fn test_arbitrum() {
    let chain = Chain {
        id: 421613,
        is_legacy: true,
    };
    test_testnet("arbitrum".into(), chain).await;
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

    println!(
        "[TEST LOG] Account 1 balance (before) = {:?}",
        balance1_before
    );
    println!(
        "[TEST LOG] Account 2 balance (before) = {:?}",
        balance2_before
    );

    let manager = {
        let database_path: String = key + "testnet_test_database.json";
        remove_file(database_path.clone()).unwrap_or(());
        let manager = Manager::new(
            net.provider.clone(),
            DefaultGasOracle::new(),
            FileSystemDatabase::new(database_path),
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
    let delta1 = (balance1_after as i64) - (balance1_before as i64);
    let delta2 = (balance2_after as i64) - (balance2_before as i64);

    println!(
        "[TEST LOG] Account 1 balance (after) = {:?}",
        balance1_after
    );
    println!(
        "[TEST LOG] Account 2 balance (after) = {:?}",
        balance2_after
    );
    println!("[TEST LOG] Delta 1 = {:?}", delta1);
    println!("[TEST LOG] Delta 2 = {:?}", delta2);

    assert!(balance1_after < balance1_before + amount);
    assert!(balance2_after == balance2_before + amount);
}
