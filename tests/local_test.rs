use ethers::{
    middleware::signer::SignerMiddleware,
    prelude::{k256::ecdsa::SigningKey, Wallet},
    providers::{Http, Provider},
    signers::{LocalWallet, Signer},
    types::Chain,
};
use serial_test::serial;
use std::{fs::remove_file, sync::Arc, time::Duration};

use tx_manager::{
    database::FileSystemDatabase,
    gas_oracle::{DefaultGasOracle, ETHGasStationOracle, GasOracle},
    manager::{Configuration, Manager},
    time::DefaultTime,
    transaction::{Priority, Transaction, Value},
};

use utilities::{assert_ok, mocks::gas_oracle::UnderpricedGasOracle, Geth};

const GETH_PORT: u16 = 8545;
const GETH_BLOCK_TIME: u16 = 12;
const GETH_CHAIN: Chain = Chain::Dev;
const PRIVATE_KEY: &str = "8da4ef21b864d2cc526dbdb2a120bd2874c36c9d0a1fb7f8c63d7f7a8b41de8f";
const FUNDS: u64 = 100;
const ETH_GAS_STATION_API_KEY: &str = "api key";
const DATABASE_PATH: &str = "./test_database.json";

ethers::contract::abigen!(TestContract, "./tests/contracts/bin/TestContract.abi");

/// Auxiliary setup function.
/// Starts the geth node and creates two accounts.
/// Then, gives FUNDS to the first account.
/// Finally, instantiates the transaction manager.
async fn geth_setup<GO: GasOracle + Send + Sync>(
    gas_oracle: GO,
    configuration: Configuration<DefaultTime>,
) -> (
    Geth,
    String,
    String,
    Manager<
        SignerMiddleware<Provider<Http>, Wallet<SigningKey>>,
        GO,
        FileSystemDatabase,
        DefaultTime,
    >,
) {
    // Starting the geth node and creating two accounts.
    let geth = Geth::start(GETH_PORT, GETH_BLOCK_TIME);
    let account1 = geth.new_account_with_private_key(PRIVATE_KEY);
    let account2 = geth.new_account();

    // Giving funds and checking account balances.
    geth.give_funds(&account1, FUNDS).await;
    assert_eq!(FUNDS, geth.check_balance_in_ethers(&account1));
    assert_eq!(0, geth.check_balance_in_ethers(&account2));

    // Instantiating the transaction manager.
    let manager = {
        let signer = PRIVATE_KEY
            .parse::<LocalWallet>()
            .unwrap()
            .with_chain_id(GETH_CHAIN);
        let provider = SignerMiddleware::new(
            Provider::<Http>::try_from(geth.url.clone()).unwrap(),
            signer,
        );
        remove_file(DATABASE_PATH).unwrap_or(());
        let manager = Manager::new(
            provider,
            gas_oracle,
            FileSystemDatabase::new(DATABASE_PATH.to_string()),
            GETH_CHAIN.into(),
            configuration,
        )
        .await;
        assert_ok!(manager);
        manager.unwrap().0
    };

    (geth, account1, account2, manager)
}

/// This test takes around 90s to finish.
#[tokio::test]
#[serial]
async fn test_manager_with_geth() {
    utilities::setup_tracing();

    let gas_oracle = ETHGasStationOracle::new(ETH_GAS_STATION_API_KEY.to_string());
    let configuration = Configuration::default();
    let (geth, account1, account2, manager) = geth_setup(gas_oracle, configuration).await;

    // Sending the first transaction.
    let amount1 = 10u64; // in ethers
    let manager = {
        let transaction = Transaction {
            from: account1.parse().unwrap(),
            to: account2.parse().unwrap(),
            value: Value::Number(ethers::utils::parse_ether(amount1).unwrap()),
            call_data: None,
        };

        let result = manager
            .send_transaction(transaction, 3, Priority::Normal)
            .await;

        assert_ok!(result);
        let (manager, _) = result.unwrap();
        let account1_balance = geth.check_balance_in_ethers(&account1);
        assert!(account1_balance == FUNDS - amount1 - 1);
        let account2_balance = geth.check_balance_in_ethers(&account2);
        assert!(account2_balance == amount1);
        manager
    };

    // Sending the second transaction
    let amount2 = 25u64; // in ethers
    {
        let transaction = Transaction {
            from: account1.parse().unwrap(),
            to: account2.parse().unwrap(),
            value: Value::Number(ethers::utils::parse_ether(amount2).unwrap()),
            call_data: None,
        };

        let result = manager
            .send_transaction(transaction, 1, Priority::ASAP)
            .await;

        assert_ok!(result);
        let (_, _) = result.unwrap();
        let account1_balance = geth.check_balance_in_ethers(&account1);
        assert!(account1_balance == FUNDS - amount1 - amount2 - 1);
        let account2_balance = geth.check_balance_in_ethers(&account2);
        assert!(account2_balance == amount1 + amount2);
    }
}

/// If you send a transaction that is exactly equal (same hash) to a transaction
/// that is already in the transaction pool, then you will receive a <code:
/// -32000, message: "already known"> error. This test checks that the
/// transaction manager ignores that error.
#[tokio::test]
#[serial]
async fn test_manager_already_known_error() {
    utilities::setup_tracing();

    // Setup.
    let configuration = Configuration::default()
        .set_transaction_mining_time(Duration::ZERO)
        .set_block_time(Duration::from_secs(GETH_BLOCK_TIME as u64 / 4));
    let gas_oracle = DefaultGasOracle::new(GETH_CHAIN.into());
    let (_geth, account1, account2, manager) = geth_setup(gas_oracle, configuration).await;

    // Sending the transaction.
    let transaction = Transaction {
        from: account1.parse().unwrap(),
        to: account2.parse().unwrap(),
        value: Value::Number(ethers::utils::parse_ether(10).unwrap()),
        call_data: None,
    };

    // Testing.
    let result = manager
        .send_transaction(transaction, 0, Priority::Normal)
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
#[serial]
async fn test_manager_transaction_underpriced_error() {
    utilities::setup_tracing();

    // Setup.
    let gas_oracle = UnderpricedGasOracle::new();
    let configuration = Configuration::default()
        .set_transaction_mining_time(Duration::ZERO)
        .set_block_time(Duration::from_secs(GETH_BLOCK_TIME as u64 / 4));
    let (_geth, account1, account2, manager) = geth_setup(gas_oracle, configuration).await;

    // Sending the transaction.
    let transaction = Transaction {
        from: account1.parse().unwrap(),
        to: account2.parse().unwrap(),
        value: Value::Number(ethers::utils::parse_ether(10).unwrap()),
        call_data: None,
    };

    // Testing.
    let result = manager
        .send_transaction(transaction, 0, Priority::Normal)
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
#[serial]
async fn test_manager_with_geth_smart_contract() {
    utilities::setup_tracing();

    let configuration = Configuration::default();
    let gas_oracle = DefaultGasOracle::new(GETH_CHAIN.into());
    let (geth, account1, _, manager) = geth_setup(gas_oracle, configuration).await;

    // Deploying the smart contract.
    let (contract_address, contract) = {
        let signer = PRIVATE_KEY
            .parse::<LocalWallet>()
            .unwrap()
            .with_chain_id(GETH_CHAIN);
        let provider = Arc::new(SignerMiddleware::new(
            Provider::<Http>::try_from(geth.url.clone()).unwrap(),
            signer,
        ));

        let contract = {
            let bytecode = hex::decode(include_bytes!("contracts/bin/TestContract.bin"))
                .unwrap()
                .into();

            let factory = ethers::prelude::ContractFactory::new(
                TESTCONTRACT_ABI.clone(),
                bytecode,
                Arc::clone(&provider),
            );
            factory.deploy(()).unwrap().send().await.unwrap()
        };
        let contract_address = contract.address();
        println!("contract_address: {}", contract_address);
        (
            contract_address,
            TestContract::new(contract_address, provider),
        )
    };

    // Sending the transaction
    // Calling the <increment> function from the smart contract.
    {
        let from = account1.parse().unwrap();
        let data = contract.increment().tx.data().unwrap().clone();
        println!("data: {}", data);
        let transaction = Transaction {
            from,
            to: contract_address,
            value: Value::Nothing,
            call_data: Some(data),
        };

        let result = manager
            .send_transaction(transaction, 1, Priority::ASAP)
            .await;

        assert_ok!(result);
        let (manager, _) = result.unwrap();
        manager
    };

    // TODO: check if the contract was updated.
}

// Auxiliary variables and functions.
