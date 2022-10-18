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

use utilities::{assert_ok, mocks::gas_oracle::UnderpricedGasOracle, Geth, ACCOUNT1, ACCOUNT2};

const CHAIN: Chain = Chain::Dev;
const FUNDS: u64 = 100;
const ETH_GAS_STATION_API_KEY: &str = "api key";
const DATABASE_PATH: &str = "./test_database.json";

ethers::contract::abigen!(TestContract, "./tests/contracts/bin/TestContract.abi");

/// This test takes around 90s to finish.
#[tokio::test]
#[serial]
async fn test_ok() {
    utilities::setup_tracing();

    let geth = init_geth(12).await;
    let manager = init_manager(
        ETHGasStationOracle::new(ETH_GAS_STATION_API_KEY.to_string()),
        Configuration::default(),
        &geth,
    )
    .await;

    // Sending the first transaction.
    let amount1 = 10u64; // in ethers
    let manager = {
        let transaction = Transaction {
            from: ACCOUNT1.into(),
            to: ACCOUNT2.into(),
            value: Value::Number(ethers::utils::parse_ether(amount1).unwrap()),
            call_data: None,
        };

        let result = manager
            .send_transaction(transaction, 3, Priority::Normal)
            .await;

        assert_ok!(result);
        let (manager, _) = result.unwrap();
        let account1_balance = geth.check_balance_in_ethers(ACCOUNT1.address);
        assert!(account1_balance == FUNDS - amount1 - 1);
        let account2_balance = geth.check_balance_in_ethers(ACCOUNT2.address);
        assert!(account2_balance == amount1);
        manager
    };

    // Sending the second transaction
    let amount2 = 25u64; // in ethers
    {
        let transaction = Transaction {
            from: ACCOUNT1.into(),
            to: ACCOUNT2.into(),
            value: Value::Number(ethers::utils::parse_ether(amount2).unwrap()),
            call_data: None,
        };

        let result = manager
            .send_transaction(transaction, 1, Priority::ASAP)
            .await;

        assert_ok!(result);
        let (_, _) = result.unwrap();
        let account1_balance = geth.check_balance_in_ethers(ACCOUNT1.address);
        assert!(account1_balance == FUNDS - amount1 - amount2 - 1);
        let account2_balance = geth.check_balance_in_ethers(ACCOUNT1.address);
        assert!(account2_balance == amount1 + amount2);
    }
}

/// If you send a transaction that is exactly equal (same hash) to a transaction
/// that is already in the transaction pool, then you will receive a <code:
/// -32000, message: "already known"> error. This test checks that the
/// transaction manager ignores that error.
#[tokio::test]
#[serial]
async fn test_error_already_known() {
    utilities::setup_tracing();

    let manager = init_manager(
        DefaultGasOracle::new(CHAIN),
        Configuration::default()
            .set_transaction_mining_time(Duration::ZERO)
            .set_block_time(Duration::from_secs(12 / 4)),
        &init_geth(12).await,
    )
    .await;

    // Sending the transaction.
    let transaction = Transaction {
        from: ACCOUNT1.into(),
        to: ACCOUNT2.into(),
        value: Value::Number(ethers::utils::parse_ether(10).unwrap()),
        call_data: None,
    };

    // Testing.
    let result = manager
        .send_transaction(transaction, 0, Priority::Normal)
        .await;
    assert!(result.is_ok());
}

/// If you send a transaction with a lower max fee than the max fee of a
/// transaction that is already in the transaction pool, then you will receive a
/// <code: -32000, message: "replacement transaction underpriced"> error. This
/// test checks that the transaction manager ignores that error.
#[tokio::test]
#[serial]
async fn test_error_transaction_underpriced() {
    utilities::setup_tracing();

    let geth = init_geth(1).await;
    let manager = init_manager(
        UnderpricedGasOracle::new(),
        Configuration::default()
            .set_transaction_mining_time(Duration::ZERO)
            .set_block_time(Duration::from_millis(900)),
        &geth,
    )
    .await;

    let transaction = Transaction {
        from: ACCOUNT1.into(),
        to: ACCOUNT2.into(),
        value: Value::Number(ethers::utils::parse_ether(10).unwrap()),
        call_data: None,
    };

    let result = manager
        .send_transaction(transaction, 0, Priority::Normal)
        .await;
    assert!(result.is_ok(), "error: {:?}", result.err().unwrap());
}

#[tokio::test]
#[serial]
async fn test_smart_contract() {
    utilities::setup_tracing();

    let geth = init_geth(12).await;
    let manager = init_manager(
        DefaultGasOracle::new(CHAIN),
        Configuration::default(),
        &geth,
    )
    .await;

    // Deploying the smart contract.
    let (contract_address, contract) = {
        let signer = ACCOUNT1
            .private_key
            .parse::<LocalWallet>()
            .unwrap()
            .with_chain_id(CHAIN);
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
        let data = contract.increment().tx.data().unwrap().clone();
        println!("data: {}", data);
        let transaction = Transaction {
            from: ACCOUNT1.into(),
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

// ------------------------------------------------------------------------------------------------
// Auxiliary
// ------------------------------------------------------------------------------------------------

// Instantiates the transaction manager.
async fn init_manager<GO: GasOracle + Send + Sync>(
    gas_oracle: GO,
    configuration: Configuration<DefaultTime>,
    geth: &Geth,
) -> Manager<
    SignerMiddleware<Provider<Http>, Wallet<SigningKey>>,
    GO,
    FileSystemDatabase,
    DefaultTime,
> {
    let signer = ACCOUNT1
        .private_key
        .parse::<LocalWallet>()
        .unwrap()
        .with_chain_id(CHAIN);
    let provider = Provider::<Http>::try_from(geth.url.clone()).unwrap();
    let provider = SignerMiddleware::new(provider, signer);
    remove_file(DATABASE_PATH).unwrap_or(());
    let database = FileSystemDatabase::new(DATABASE_PATH.to_string());
    let manager = Manager::new(provider, gas_oracle, database, CHAIN, configuration).await;
    assert_ok!(manager);
    manager.unwrap().0
}

/// Starts geth and gives FUNDS to ACCOUNT1.
async fn init_geth(block_time: u16) -> Geth {
    // Starting the geth node and creating two accounts.
    let geth = Geth::start(8545, block_time);

    // Giving funds and checking account balances.
    geth.give_funds(ACCOUNT1.address, FUNDS).await;
    assert_eq!(FUNDS, geth.check_balance_in_ethers(ACCOUNT1.address));
    assert_eq!(0, geth.check_balance_in_ethers(ACCOUNT2.address));

    geth
}
