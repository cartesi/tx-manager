use ethers::types::Chain;
use std::{fs::remove_file, time::Duration};

use tx_manager::{
    database::FileSystemDatabase,
    gas_oracle::DefaultGasOracle,
    manager::{Configuration, Manager},
    transaction::{Priority, Transaction, Value},
};

use utilities::{Net, TestConfiguration, TEST_CONFIGURATION_PATH};

const PROVIDER_HTTP_URL: &str = "https://goerli.infura.io/v3";

/*
#[tokio::test]
async fn test_todo() {
    let net = Net::new(
        PROVIDER_HTTP_URL.to_string() + INFURA_API_KEY,
        Chain::Goerli,
        ACCOUNT1,
    );

    let balance1 = net.get_balance_in_gwei(ACCOUNT1).await;
    let balance2 = net.get_balance_in_gwei(ACCOUNT2).await;

    println!("Wallet 1 balance (in gwei): {:?}", balance1);
    println!("Wallet 2 balance (in gwei): {:?}", balance2);

    let (max_fee, max_priority_fee) = net.provider.estimate_eip1559_fees(None).await.unwrap();
    println!("max_fee: {:?}", utilities::wei_to_gwei(max_fee));
    println!(
        "max_priority_fee: {:?}",
        utilities::wei_to_gwei(max_priority_fee)
    );

    todo!()
}
*/

#[tokio::test]
async fn test_goerli() {
    utilities::setup_tracing();

    let test_configuration = TestConfiguration::get(TEST_CONFIGURATION_PATH.into());
    let account1 = test_configuration.account1;
    let account2 = test_configuration.account2;

    let net = Net::new(
        format!(
            "{}/{}",
            PROVIDER_HTTP_URL, test_configuration.infura_api_key
        ),
        Chain::Goerli,
        &account1,
    );

    let balance1_before = net.get_balance_in_gwei(&account1).await;
    let balance2_before = net.get_balance_in_gwei(&account2).await;

    let manager = {
        const DATABASE_PATH: &str = "./test_live_database.json";
        remove_file(DATABASE_PATH).unwrap_or(());
        let manager = Manager::new(
            net.provider.clone(),
            DefaultGasOracle::new(net.chain),
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
