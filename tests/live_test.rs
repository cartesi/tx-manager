/*
use ethers::{providers::Middleware, signers::Signer, types::Chain};
use serial_test::serial;
use std::fs::remove_file;

use tx_manager::{
    database::FileSystemDatabase,
    gas_oracle::ETHGasStationOracle,
    manager::{Configuration, Manager},
    transaction::{Priority, Transaction, Value},
};

mod infra;
use infra::Net;


#[tokio::test]
async fn test_goerli() {
    // setup_tracing(); TODO

    let provider_http_url = "https://goerli.infura.io/v3/".to_string() + infura_api_key;
    let net = Net {
        provider_http_url,
        chain: Chain::Goerli,
    };

    let wallet1 = net.create_wallet(PRIVATE_KEY1);
    let wallet2 = net.create_wallet(PRIVATE_KEY2);

    println!("Wallet 1: {:?}", wallet1);
    println!("Wallet 2: {:?}", wallet2);

    let provider = net.provider(&wallet1);

    let balance1_before = provider.get_balance(wallet1.address(), None).await.unwrap();
    let balance2_before = provider.get_balance(wallet2.address(), None).await.unwrap();

    println!("[BEFORE] Wallet 1 balance: {:?}", balance1_before);
    println!("[BEFORE] Wallet 2 balance: {:?}", balance2_before);

    let manager = {
        const DATABASE_PATH: &str = "./test_live_database.json";
        remove_file(DATABASE_PATH).unwrap_or(());
        let manager = Manager::new(
            provider.clone(),
            None as Option<ETHGasStationOracle>,
            FileSystemDatabase::new(DATABASE_PATH.into()),
            net.chain,
            Configuration::default(),
        )
        .await;
        assert!(manager.is_ok());
        manager.unwrap().0
    };

    let transaction = Transaction {
        from: wallet1.address(),
        to: wallet2.address(),
        value: Value::Number((10e12 as u64).into()),
        call_data: None,
    };

    let result = manager
        .send_transaction(transaction, 1, Priority::Normal)
        .await;
    assert!(result.is_ok(), "err: {}", result.err().unwrap());

    let balance1_after = provider.get_balance(wallet1.address(), None).await.unwrap();
    let balance2_after = provider.get_balance(wallet2.address(), None).await.unwrap();

    println!("[AFTER] Wallet 1 balance: {:?}", balance1_after);
    println!("[AFTER] Wallet 2 balance: {:?}", balance2_after);
    println!(
        "Gas cost: {:?}",
        balance2_after
            .checked_sub(balance1_before)
            .unwrap()
            .checked_add(1000.into())
            .unwrap()
    );
}
*/
