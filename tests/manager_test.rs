use ethers::middleware::signer::SignerMiddleware;
use ethers::prelude::k256::ecdsa::SigningKey;
use ethers::prelude::Wallet;
use ethers::providers::{Http, Provider};
use ethers::signers::LocalWallet;
use ethers::signers::Signer;
use ethers::types::{TransactionReceipt, U256, U64};
use serial_test::serial;
use std::fs::remove_file;
use std::sync::Arc;
use std::time::Duration;
use tracing_subscriber::filter::EnvFilter;

use tx_manager::database::FileSystemDatabase;
use tx_manager::gas_oracle::{EIP1559GasInfo, ETHGasStationOracle, GasInfo, GasOracleInfo};
use tx_manager::manager::{Configuration, Manager};
use tx_manager::time::DefaultTime;
use tx_manager::transaction::{
    PersistentState, Priority, StaticTxData, SubmittedTxs, Transaction, Value,
};

mod utils;
use utils::{
    Database, DatabaseStateError, GasOracle, GasOracleError, GethNode, IncrementingGasOracle,
    MockMiddleware, MockMiddlewareError, Time, UnderpricedGasOracle,
};

use tx_manager::gas_oracle::GasOracle as GasOracleTrait;

type MockManagerError = tx_manager::Error<MockMiddleware, GasOracle, Database>;
type MockManagerError2<GO> = tx_manager::Error<MockMiddleware, GO, Database>;

macro_rules! assert_ok(
    ($result: expr) => {
        match $result {
            Ok(..) => {},
            Err(err) => panic!("expected Ok, got Err({:?})", err),
        }
    };
);

macro_rules! assert_err(
    ($result: expr, $expected: expr) => {
        match $result {
            Ok(..) => panic!("expected Err({:?}), got Ok(..)", $expected),
            Err(err) => assert_eq!(err.to_string(), $expected.to_string()),
        }
    };
);

const GETH_PORT: u16 = 8545;
const GETH_BLOCK_TIME: u16 = 12;
const GETH_CHAIN_ID: u64 = 1337;
const PRIVATE_KEY: &str = "8da4ef21b864d2cc526dbdb2a120bd2874c36c9d0a1fb7f8c63d7f7a8b41de8f";
const FUNDS: u64 = 100;
const ETH_GAS_STATION_API_KEY: &str = "api key";

ethers::contract::abigen!(TestContract, "./tests/contracts/bin/TestContract.abi");

/// Auxiliary setup function.
/// Starts the geth node and creates two accounts.
/// Then, gives FUNDS to the first account.
/// Finally, instantiates the transaction manager.
async fn geth_setup<GO: tx_manager::gas_oracle::GasOracle>(
    gas_oracle: Option<GO>,
    configuration: Configuration<DefaultTime>,
) -> (
    GethNode,
    String,
    String,
    Manager<
        SignerMiddleware<Provider<Http>, Wallet<SigningKey>>,
        GO,
        FileSystemDatabase,
        DefaultTime,
    >,
)
where
    GO: Send + Sync,
{
    // Starting the geth node and creating two accounts.
    let geth = GethNode::start(GETH_PORT, GETH_BLOCK_TIME);
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
            .with_chain_id(GETH_CHAIN_ID);
        let provider = SignerMiddleware::new(
            Provider::<Http>::try_from(geth.url.clone()).unwrap(),
            signer,
        );
        let database_path = "./test_database.json";
        remove_file(database_path).unwrap_or(());
        let manager = Manager::new(
            provider,
            gas_oracle,
            FileSystemDatabase::new(database_path.to_string()),
            GETH_CHAIN_ID.into(),
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
    setup_tracing();

    let gas_oracle = ETHGasStationOracle::new(ETH_GAS_STATION_API_KEY.to_string());
    let configuration = Configuration::default();
    let (geth, account1, account2, manager) = geth_setup(Some(gas_oracle), configuration).await;

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
    setup_tracing();

    // Setup.
    let configuration = Configuration::default()
        .set_transaction_mining_time(Duration::ZERO)
        .set_block_time(Duration::from_secs(GETH_BLOCK_TIME as u64 / 4));
    let (_geth, account1, account2, manager) = geth_setup::<GasOracle>(None, configuration).await;

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
    setup_tracing();

    // Setup.
    let gas_oracle = UnderpricedGasOracle::new();
    let configuration = Configuration::default()
        .set_transaction_mining_time(Duration::ZERO)
        .set_block_time(Duration::from_secs(GETH_BLOCK_TIME as u64 / 4));
    let (_geth, account1, account2, manager) = geth_setup(Some(gas_oracle), configuration).await;

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
    setup_tracing();

    let configuration = Configuration::default();
    let (geth, account1, _, manager) = geth_setup::<GasOracle>(None, configuration).await;

    // Deploying the smart contract.
    let (contract_address, contract) = {
        let signer = PRIVATE_KEY
            .parse::<LocalWallet>()
            .unwrap()
            .with_chain_id(GETH_CHAIN_ID);
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

#[tokio::test]
#[serial]
async fn test_manager_new() {
    setup_tracing();
    let chain_id = U64::from(1u64);

    let transaction = Transaction {
        from: HASH1.parse().unwrap(),
        to: HASH2.parse().unwrap(),
        value: Value::Number(U256::from(5u64)),
        call_data: None,
    };

    // Instantiating a new transaction manager that has no pending transactions.
    {
        let (middleware, gas_oracle, mut db) = setup_dependencies();
        db.get_state_output = Some(None);
        let result = Manager::new(
            middleware,
            Some(gas_oracle),
            db,
            chain_id,
            Configuration::default(),
        )
        .await;
        assert_ok!(result);
        let (_, transaction_receipt) = result.unwrap();
        assert_eq!(transaction_receipt, None);
    }

    // Trying to instantiate new transaction manager without being able to check if
    // there is a transaction pending.
    {
        let (middleware, gas_oracle, mut db) = setup_dependencies();
        db.get_state_output = None;
        let result = Manager::new(
            middleware,
            Some(gas_oracle),
            db,
            chain_id,
            Configuration::default(),
        )
        .await;
        let expected_err: MockManagerError = tx_manager::Error::Database(DatabaseStateError::Get);
        assert_err!(result, expected_err);
    }

    // Instantiating a new transaction manager that has one pending transaction.
    // The pending transaction's hash is transaction_hash[0].
    {
        let (mut middleware, gas_oracle, mut db) = setup_dependencies();
        middleware.get_block_number = vec![1];
        middleware.get_transaction_receipt = vec![true];
        middleware.get_transaction_count = Some(());
        db.get_state_output = Some(Some(PersistentState {
            tx_data: StaticTxData {
                nonce: 1u64.into(),
                transaction: transaction.clone(),
                priority: Priority::Normal,
                confirmations: 1,
            },
            submitted_txs: SubmittedTxs {
                txs_hashes: vec![TRANSACTION_HASH1.parse().unwrap()],
            },
        }));
        db.clear_state_output = Some(());
        let result = Manager::new(
            middleware,
            Some(gas_oracle),
            db,
            chain_id,
            Configuration {
                transaction_mining_time: Duration::ZERO,
                block_time: Duration::ZERO,
                time: Time,
                legacy: false,
            },
        )
        .await;
        assert_ok!(result);
        let (_, transaction_receipt) = result.unwrap();
        assert!(transaction_receipt.is_some());
    };

    // Trying to instantiate a new transaction manager that has one pending
    // transaction without being able to clear the state after the confirmation.
    // The pending transaction's hash is transaction_hash[0].
    {
        let (mut middleware, gas_oracle, mut db) = setup_dependencies();
        middleware.get_block_number = vec![1];
        middleware.get_transaction_receipt = vec![true];
        middleware.get_transaction_count = Some(());
        db.get_state_output = Some(Some(PersistentState {
            tx_data: StaticTxData {
                nonce: 1u64.into(),
                transaction: transaction.clone(),
                priority: Priority::Normal,
                confirmations: 1,
            },
            submitted_txs: SubmittedTxs {
                txs_hashes: vec![TRANSACTION_HASH1.parse().unwrap()],
            },
        }));
        let result = Manager::new(
            middleware,
            Some(gas_oracle),
            db,
            chain_id,
            Configuration {
                transaction_mining_time: Duration::ZERO,
                block_time: Duration::ZERO,
                time: Time,
                legacy: false,
            },
        )
        .await;
        let expected_err: MockManagerError = tx_manager::Error::Database(DatabaseStateError::Clear);
        assert_err!(result, expected_err);
    }
}

#[tokio::test]
#[serial]
async fn test_manager_send_transaction_advanced() {
    setup_tracing();

    // Resends the transaction once.
    {
        let result = run_send_transaction2(1, IncrementingGasOracle::new(), |mut middleware| {
            middleware.get_block_number = vec![1];
            middleware.get_transaction_receipt = vec![false, true];
            middleware
        })
        .await;
        assert_ok!(result);

        assert_eq!(0, MockMiddleware::global().estimate_eip1559_fees_n);
        assert_eq!(1, MockMiddleware::global().get_block_number_n);
        assert_eq!(1, MockMiddleware::global().get_transaction_count_n);
        assert_eq!(2, MockMiddleware::global().estimate_gas_n);
        assert_eq!(2, MockMiddleware::global().sign_transaction_n);
        assert_eq!(2, MockMiddleware::global().send_raw_transaction_n);
        assert_eq!(2, MockMiddleware::global().get_transaction_receipt_n);
    }

    // Resends the transaction twice.
    {
        let result = run_send_transaction2(1, IncrementingGasOracle::new(), |mut middleware| {
            middleware.get_block_number = vec![1];
            middleware.get_transaction_receipt = vec![false, false, false, true];
            middleware
        })
        .await;
        assert_ok!(result);

        assert_eq!(0, MockMiddleware::global().estimate_eip1559_fees_n);
        assert_eq!(1, MockMiddleware::global().get_block_number_n);
        assert_eq!(1, MockMiddleware::global().get_transaction_count_n);
        assert_eq!(3, MockMiddleware::global().estimate_gas_n);
        assert_eq!(3, MockMiddleware::global().sign_transaction_n);
        assert_eq!(3, MockMiddleware::global().send_raw_transaction_n);
        assert_eq!(4, MockMiddleware::global().get_transaction_receipt_n);
    }
}

#[tokio::test]
#[serial]
async fn test_manager_send_transaction_basic() {
    setup_tracing();

    // Ok (0 confirmations).
    {
        let result =
            run_send_transaction(0, |middleware, gas_oracle, db| (middleware, gas_oracle, db))
                .await;
        assert_ok!(result);

        assert_eq!(0, MockMiddleware::global().estimate_eip1559_fees_n);
        assert_eq!(1, MockMiddleware::global().get_block_number_n);
        assert_eq!(1, MockMiddleware::global().get_transaction_count_n);
        assert_eq!(1, MockMiddleware::global().estimate_gas_n);
        assert_eq!(1, MockMiddleware::global().sign_transaction_n);
        assert_eq!(1, MockMiddleware::global().send_raw_transaction_n);
        assert_eq!(1, MockMiddleware::global().get_transaction_receipt_n);
    }

    // Ok (1 confirmation).
    {
        let result = run_send_transaction(1, |mut middleware, gas_oracle, db| {
            middleware.get_block_number = vec![1];
            middleware.get_transaction_receipt = vec![true];
            (middleware, gas_oracle, db)
        })
        .await;
        assert_ok!(result);

        assert_eq!(0, MockMiddleware::global().estimate_eip1559_fees_n);
        assert_eq!(1, MockMiddleware::global().get_block_number_n);
        assert_eq!(1, MockMiddleware::global().get_transaction_count_n);
        assert_eq!(1, MockMiddleware::global().estimate_gas_n);
        assert_eq!(1, MockMiddleware::global().sign_transaction_n);
        assert_eq!(1, MockMiddleware::global().send_raw_transaction_n);
        assert_eq!(1, MockMiddleware::global().get_transaction_receipt_n);
    }

    // Ok (2 confirmations).
    {
        let result = run_send_transaction(2, |mut middleware, gas_oracle, db| {
            middleware.get_block_number = vec![1, 1, 1, 2];
            middleware.get_transaction_receipt = vec![true; 4];
            (middleware, gas_oracle, db)
        })
        .await;
        assert_ok!(result);

        assert_eq!(0, MockMiddleware::global().estimate_eip1559_fees_n);
        assert_eq!(4, MockMiddleware::global().get_block_number_n);
        assert_eq!(1, MockMiddleware::global().get_transaction_count_n);
        assert_eq!(1, MockMiddleware::global().estimate_gas_n);
        assert_eq!(1, MockMiddleware::global().sign_transaction_n);
        assert_eq!(1, MockMiddleware::global().send_raw_transaction_n);
        assert_eq!(4, MockMiddleware::global().get_transaction_receipt_n);
    }

    // Ok (10 confirmations).
    {
        let result = run_send_transaction(10, |mut middleware, gas_oracle, db| {
            middleware.get_block_number = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
            middleware.get_transaction_receipt = vec![true; 10];
            (middleware, gas_oracle, db)
        })
        .await;
        assert_ok!(result);

        assert_eq!(10, MockMiddleware::global().get_block_number_n);
        assert_eq!(1, MockMiddleware::global().get_transaction_count_n);
        assert_eq!(1, MockMiddleware::global().estimate_gas_n);
        assert_eq!(1, MockMiddleware::global().sign_transaction_n);
        assert_eq!(1, MockMiddleware::global().send_raw_transaction_n);
        assert_eq!(10, MockMiddleware::global().get_transaction_receipt_n);
    }
}

#[tokio::test]
#[serial]
async fn test_manager_send_transaction_basic_middleware_errors() {
    setup_tracing();

    // "Middleware::estimate_eip1559_fees" is being tested in the
    // test_manager_send_transaction_basic_gas_oracle_errors function bellow.

    // When "Middleware::get_transaction_count" fails.
    {
        let result = run_send_transaction(0, |mut middleware, gas_oracle, db| {
            middleware.get_transaction_count = None;
            (middleware, gas_oracle, db)
        })
        .await;
        let expected_err: MockManagerError =
            tx_manager::Error::Middleware(MockMiddlewareError::GetTransactionCount);
        assert_err!(result, expected_err);
        assert_eq!(1, MockMiddleware::global().get_transaction_count_n)
    }

    // When "Middleware::estimate_gas" fails.
    {
        let result = run_send_transaction(0, |mut middleware, gas_oracle, db| {
            middleware.estimate_gas = None;
            (middleware, gas_oracle, db)
        })
        .await;
        let expected_err: MockManagerError =
            tx_manager::Error::Middleware(MockMiddlewareError::EstimateGas);
        assert_err!(result, expected_err);
        assert_eq!(1, MockMiddleware::global().estimate_gas_n)
    }

    // When "Middleware::sign_transaction" fails.
    {
        let result = run_send_transaction(0, |mut middleware, gas_oracle, db| {
            middleware.sign_transaction = None;
            (middleware, gas_oracle, db)
        })
        .await;
        let expected_err: MockManagerError =
            tx_manager::Error::Middleware(MockMiddlewareError::SignTransaction);
        assert_err!(result, expected_err);
        assert_eq!(1, MockMiddleware::global().sign_transaction_n)
    }

    // When "Middleware::send_transaction" fails.
    {
        let result = run_send_transaction(0, |mut middleware, gas_oracle, db| {
            middleware.send_transaction = None;
            (middleware, gas_oracle, db)
        })
        .await;
        let expected_err: MockManagerError =
            tx_manager::Error::Middleware(MockMiddlewareError::SendTransaction);
        assert_err!(result, expected_err);
        assert_eq!(1, MockMiddleware::global().send_raw_transaction_n)
    }

    // When "Middleware::get_transaction_receipt" fails.
    {
        let result = run_send_transaction(0, |mut middleware, gas_oracle, db| {
            middleware.get_transaction_receipt = vec![];
            (middleware, gas_oracle, db)
        })
        .await;
        let expected_err: MockManagerError =
            tx_manager::Error::Middleware(MockMiddlewareError::GetTransactionReceipt(1));
        assert_err!(result, expected_err);
        assert_eq!(1, MockMiddleware::global().get_transaction_receipt_n)
    }

    // When "Middleware::get_block_number" fails.
    {
        let result = run_send_transaction(0, |mut middleware, gas_oracle, db| {
            middleware.get_block_number = vec![];
            (middleware, gas_oracle, db)
        })
        .await;
        let expected_err: MockManagerError =
            tx_manager::Error::Middleware(MockMiddlewareError::GetBlockNumber);
        assert_err!(result, expected_err);
        assert_eq!(1, MockMiddleware::global().get_block_number_n)
    }
}

#[tokio::test]
#[serial]
async fn test_manager_send_transaction_basic_gas_oracle_errors() {
    setup_tracing();

    // When only "GasOracle::gas_info" fails.
    {
        let result = run_send_transaction(0, |mut middleware, mut gas_oracle, db| {
            middleware.estimate_eip1559_fees = Some((300, 50));
            gas_oracle.gas_oracle_info_output = None;
            (middleware, gas_oracle, db)
        })
        .await;
        assert_ok!(result);
        assert_eq!(1, GasOracle::global().gas_info_n);
        assert_eq!(1, MockMiddleware::global().estimate_eip1559_fees_n);
    }

    // When both "GasOracle::gas_info" and
    // "Middleware::estimate_eip1559_fees" fail.
    {
        let result = run_send_transaction(0, |middleware, mut gas_oracle, db| {
            gas_oracle.gas_oracle_info_output = None;
            (middleware, gas_oracle, db)
        })
        .await;
        let expected_err: MockManagerError = tx_manager::Error::GasOracle(
            GasOracleError::GasInfo,
            MockMiddlewareError::EstimateEIP1559Fees,
        );
        assert_err!(result, expected_err);
        assert_eq!(1, GasOracle::global().gas_info_n);
        assert_eq!(1, MockMiddleware::global().estimate_eip1559_fees_n);
    }
}

#[tokio::test]
#[serial]
async fn test_manager_send_transaction_basic_database_errors() {
    setup_tracing();

    // When "Database::set_state" fails.
    {
        let result = run_send_transaction(0, |middleware, gas_oracle, mut db| {
            db.set_state_output = None;
            (middleware, gas_oracle, db)
        })
        .await;
        let expected_err: MockManagerError = tx_manager::Error::Database(DatabaseStateError::Set);
        assert_err!(result, expected_err);
        assert_eq!(1, Database::global().set_state_n);
    }

    // When "Database::clear_state" fails.
    {
        let result = run_send_transaction(0, |middleware, gas_oracle, mut db| {
            db.clear_state_output = None;
            (middleware, gas_oracle, db)
        })
        .await;
        let expected_err: MockManagerError = tx_manager::Error::Database(DatabaseStateError::Clear);
        assert_err!(result, expected_err);
        assert_eq!(1, Database::global().clear_state_n);
    }
}

// Auxiliary variables and functions.

const HASH1: &str = "0xba763b97851b653aaaf631723bab41a500f03b29";
const HASH2: &str = "0x29e425df042e83e4ddb3ee3348d6d745c58fce8f";

const TRANSACTION_HASH1: &str =
    "0x2b34df791cc4eb898f6d4437713e946f216cac6a3921b2899db919abe26739b2";

fn setup_tracing() {
    let format = tracing_subscriber::fmt::format()
        .without_time()
        .with_target(false)
        .with_level(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false)
        .with_source_location(false)
        .compact();
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .event_format(format)
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();
}

fn setup_dependencies() -> (MockMiddleware, GasOracle, Database) {
    (MockMiddleware::new(), GasOracle::new(), Database::new())
}

async fn setup_manager<GO: tx_manager::gas_oracle::GasOracle>(
    middleware: MockMiddleware,
    gas_oracle: GO,
    mut db: Database,
) -> Manager<MockMiddleware, GO, Database, Time>
where
    GO: Send + Sync,
{
    db.get_state_output = Some(None);
    let result = Manager::new(
        middleware,
        Some(gas_oracle),
        db,
        U64::from(1), // chain id
        Configuration {
            transaction_mining_time: Duration::ZERO,
            block_time: Duration::ZERO,
            time: Time,
            legacy: false,
        },
    )
    .await;
    assert_ok!(result);
    let (manager, transaction_receipt) = result.unwrap();
    assert!(transaction_receipt.is_none());
    manager
}

fn setup_middleware(mut middleware: MockMiddleware) -> MockMiddleware {
    middleware.estimate_gas = Some(U256::from(21000));
    middleware.get_block_number = vec![1];
    middleware.get_transaction_count = Some(());
    middleware.get_transaction_receipt = vec![true];
    middleware.send_transaction = Some(());
    middleware.sign_transaction = Some(());
    middleware
}

async fn run_send_transaction(
    confirmations: usize,
    f: fn(MockMiddleware, GasOracle, Database) -> (MockMiddleware, GasOracle, Database),
) -> Result<TransactionReceipt, MockManagerError> {
    let (mut middleware, mut gas_oracle, mut db) = setup_dependencies();
    middleware = setup_middleware(middleware);
    gas_oracle.gas_oracle_info_output = Some(GasOracleInfo {
        gas_info: GasInfo::EIP1559(EIP1559GasInfo {
            max_fee: U256::from(1_000_000_000),
            max_priority_fee: Some(U256::from(100_000)),
        }),
        mining_time: None,
        block_time: None,
    });
    db.get_state_output = None;
    db.set_state_output = Some(());
    db.clear_state_output = Some(());
    let (middleware, gas_oracle, db) = f(middleware, gas_oracle, db);

    let manager = setup_manager(middleware, gas_oracle, db).await;
    let transaction = Transaction {
        from: HASH1.parse().unwrap(),
        to: HASH2.parse().unwrap(),
        value: Value::Number(U256::from(5u64)),
        call_data: None,
    };
    manager
        .send_transaction(transaction, confirmations, Priority::Normal)
        .await
        .map(|(_, receipt)| receipt)
}

// TODO
async fn run_send_transaction2<GO: GasOracleTrait>(
    confirmations: usize,
    gas_oracle: GO,
    f: fn(MockMiddleware) -> MockMiddleware,
) -> Result<TransactionReceipt, MockManagerError2<GO>>
where
    GO: Send + Sync,
{
    let (mut middleware, _, mut db) = setup_dependencies();
    middleware = setup_middleware(middleware);
    db.get_state_output = None;
    db.set_state_output = Some(());
    db.clear_state_output = Some(());
    let middleware = f(middleware);

    let manager = setup_manager(middleware, gas_oracle, db).await;
    let transaction = Transaction {
        from: HASH1.parse().unwrap(),
        to: HASH2.parse().unwrap(),
        value: Value::Number(U256::from(5u64)),
        call_data: None,
    };
    manager
        .send_transaction(transaction, confirmations, Priority::Normal)
        .await
        .map(|(_, receipt)| receipt)
}
