use ethers::{
    middleware::signer::SignerMiddleware,
    prelude::{k256::ecdsa::SigningKey, Wallet},
    providers::{Http, Provider},
};
use serial_test::serial;
use std::{fs::remove_file, sync::Arc, time::Duration};

use tx_manager::{
    database::FileSystemDatabase,
    gas_oracle::{DefaultGasOracle, GasOracle},
    manager::{Configuration, Manager},
    time::DefaultTime,
    transaction::{Priority, Transaction, Value},
    Chain,
};

use utilities::{
    assert_ok,
    mocks::gas_oracle::{ConstantGasOracle, UnderpricedGasOracle},
    Account, Geth,
};

const CHAIN: Chain = Chain {
    id: 1337,
    is_legacy: false,
};
const FUNDS: u64 = 1e9 as u64;
const DATABASE_PATH: &str = "./test_database.json";

ethers::contract::abigen!(TestContract, "./tests/contracts/bin/TestContract.abi");

#[tokio::test]
#[serial]
async fn test_ok() {
    utilities::setup_tracing();

    let (geth, account1, account2, manager) = init(
        1,
        DefaultGasOracle::new(),
        Configuration::default().set_block_time(Duration::from_secs(1)),
    )
    .await;

    // Sending the first transaction.
    let manager = {
        let balance = geth
            .provider
            .get_balance(account1.clone(), account2.clone())
            .await;

        let amount1 = 10u64;
        let transaction = Transaction {
            from: account1.clone().into(),
            to: account2.clone().into(),
            value: Value::Number(utilities::gwei_to_wei(amount1)),
            call_data: None,
        };

        let result = manager
            .send_transaction(transaction, 3, Priority::Normal)
            .await;

        assert_ok!(result);
        let (manager, _) = result.unwrap();

        geth.provider
            .check_transaction_balance(balance, amount1)
            .await;

        manager
    };

    // Sending the second transaction
    {
        let balance = geth
            .provider
            .get_balance(account1.clone(), account2.clone())
            .await;

        let amount2 = 25u64;
        let transaction = Transaction {
            from: account1.clone().into(),
            to: account2.clone().into(),
            value: Value::Number(utilities::gwei_to_wei(amount2)),
            call_data: None,
        };

        let result = manager
            .send_transaction(transaction, 1, Priority::ASAP)
            .await;

        assert_ok!(result);
        let (_, _) = result.unwrap();

        geth.provider
            .check_transaction_balance(balance, amount2)
            .await;
    }
}

#[tokio::test]
#[serial]
async fn test_smart_contract() {
    utilities::setup_tracing();

    let (geth, account1, _, manager) = init(
        1,
        DefaultGasOracle::new(),
        Configuration::default().set_block_time(Duration::from_secs(1)),
    )
    .await;

    // Deploying the smart contract.
    let (contract_address, contract) = {
        let provider = Arc::new(geth.provider.inner.clone());
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
        let data = contract.increment().tx.data().unwrap().clone();
        println!("data: {}", data);
        let transaction = Transaction {
            from: account1.clone().into(),
            to: contract_address,
            value: Value::Nothing,
            call_data: Some(data),
        };

        let result = manager
            .send_transaction(transaction, 0, Priority::ASAP)
            .await;

        assert_ok!(result);
        let (manager, _) = result.unwrap();
        manager
    };

    // TODO: check if the contract was updated.
}

/// If you send a transaction that is exactly equal (same hash) to a transaction
/// that is already in the transaction pool, then you will receive a <code:
/// -32000, message: "already known"> error. This test checks that the
/// transaction manager ignores that error.
#[tokio::test]
#[serial]
async fn test_error_already_known() {
    utilities::setup_tracing();

    let (geth, account1, account2, manager) = init(
        1,
        ConstantGasOracle::new(),
        Configuration::default()
            .set_transaction_mining_time(Duration::ZERO)
            .set_block_time(Duration::from_millis(800)),
    )
    .await;

    let balance = geth
        .provider
        .get_balance(account1.clone(), account2.clone())
        .await;

    let amount = 100;
    let transaction = Transaction {
        from: account1.clone().into(),
        to: account2.clone().into(),
        value: Value::Number(utilities::gwei_to_wei(amount)),
        call_data: None,
    };

    let result = manager
        .send_transaction(transaction, 0, Priority::Normal)
        .await;
    assert_ok!(result);

    geth.provider
        .check_transaction_balance(balance, amount)
        .await;

    // TODO: check for "already known" in the logs.
}

/// If you send a transaction with a lower max fee than the max fee of a
/// transaction that is already in the transaction pool, then you will receive a
/// <code: -32000, message: "replacement transaction underpriced"> error. This
/// test checks that the transaction manager ignores that error.
#[tokio::test]
#[serial]
async fn test_error_transaction_underpriced() {
    utilities::setup_tracing();

    let (geth, account1, account2, manager) = init(
        1,
        UnderpricedGasOracle::new(),
        Configuration::default()
            .set_transaction_mining_time(Duration::ZERO)
            .set_block_time(Duration::from_millis(800)),
    )
    .await;

    let balance = geth
        .provider
        .get_balance(account1.clone(), account2.clone())
        .await;

    let amount = 10;
    let transaction = Transaction {
        from: account1.clone().into(),
        to: account2.clone().into(),
        value: Value::Number(utilities::gwei_to_wei(amount)),
        call_data: None,
    };

    let result = manager
        .send_transaction(transaction, 0, Priority::Normal)
        .await;
    assert_ok!(result);

    geth.provider
        .check_transaction_balance(balance, amount)
        .await;

    // TODO: check for "transaction underpriced" in the logs.
}

// ------------------------------------------------------------------------------------------------
// Auxiliary
// ------------------------------------------------------------------------------------------------

/// Creates account1 and account2, starts geth, gives FUNDS to account1, and
/// instantiates the transaction manager.
async fn init<GO: GasOracle + Send + Sync>(
    block_time: u16,
    gas_oracle: GO,
    configuration: Configuration<DefaultTime>,
) -> (
    Geth,
    Account,
    Account,
    Manager<
        SignerMiddleware<Provider<Http>, Wallet<SigningKey>>,
        GO,
        FileSystemDatabase,
        DefaultTime,
    >,
) {
    let account1 = Account::random();
    let account2 = Account::random();

    // Starting the geth node and creating two accounts.
    let geth = Geth::start(8545, block_time, CHAIN, &account1);

    // Giving funds and checking account balances.
    geth.give_funds(&account1, FUNDS).await;
    assert_eq!(FUNDS, geth.provider.get_balance_in_gwei(&account1).await);
    assert_eq!(0, geth.provider.get_balance_in_gwei(&account2).await);

    remove_file(DATABASE_PATH).unwrap_or(());
    let database = FileSystemDatabase::new(DATABASE_PATH.to_string());
    let manager = Manager::new(
        geth.provider.inner.clone(),
        gas_oracle,
        database,
        CHAIN,
        configuration,
    )
    .await;
    assert_ok!(manager);
    let manager = manager.unwrap().0;

    (geth, account1, account2, manager)
}
