use ethers::middleware::signer::SignerMiddleware;
use ethers::providers::{Http, Provider};
use ethers::signers::LocalWallet;
use ethers::signers::Signer;
use ethers::types::{TransactionReceipt, U256, U64};
use serial_test::serial;
use std::fs::remove_file;
use std::time::Duration;

use tx_manager::database::FileSystemDatabase;
use tx_manager::gas_oracle::{ETHGasStationOracle, GasInfo};
use tx_manager::manager::{Configuration, Manager, ManagerError, State};
use tx_manager::time::DefaultTime;
use tx_manager::transaction::{Priority, Transaction, Value};

mod utils;
use utils::{
    Database, DatabaseError, GasOracle, GasOracleError, GethNode,
    MockMiddleware, MockMiddlewareError, Time,
};

type MockManagerError = ManagerError<MockMiddleware, GasOracle, Database>;

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

/// This test takes around 90s to finish.
#[tokio::test]
async fn test_manager_with_geth() {
    setup_tracing();

    // Geth setup.
    let chain_id = 1337u64;
    let private_key =
        "8da4ef21b864d2cc526dbdb2a120bd2874c36c9d0a1fb7f8c63d7f7a8b41de8f"
            .to_owned();
    let geth = GethNode::start(8545, 12);
    let account1: String = geth.new_account_with_private_key(&private_key);
    let account2: String = geth.new_account();
    const INITIAL_FUNDS: u64 = 100;
    geth.give_funds(&account1, INITIAL_FUNDS);

    // Waiting for the funds to be credited.
    let account2_balance = geth.check_balance_in_ethers(&account2);
    assert!(account2_balance == 0);
    loop {
        let account1_balance = geth.check_balance_in_ethers(&account1);
        if account1_balance == 100 {
            break;
        } else {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    // Instantiating the manager.
    let manager = {
        let provider = Provider::<Http>::try_from(geth.url.clone()).unwrap();
        let signer: LocalWallet = private_key.parse().unwrap();
        let signer = signer.with_chain_id(chain_id);
        let provider = SignerMiddleware::new(provider, signer);
        let gas_oracle = ETHGasStationOracle::new("api key");
        let database_path = "./test_database.json";
        let _ = remove_file(database_path);
        let database = FileSystemDatabase::new(database_path);
        let result = Manager::new(
            provider,
            gas_oracle,
            database,
            chain_id.into(),
            Configuration {
                transaction_mining_time: Duration::from_secs(1),
                block_time: Duration::from_secs(1),
                time: DefaultTime,
            },
        )
        .await;
        assert_ok!(result);
        let (manager, _) = result.unwrap();
        manager
    };

    // Sending the first transaction.
    let amount1 = 10u64; // in ethers
    let manager = {
        let transaction = Transaction {
            priority: Priority::Normal,
            from: account1.parse().unwrap(),
            to: account2.parse().unwrap(),
            value: Value::Number(ethers::utils::parse_ether(amount1).unwrap()),
            confirmations: 3,
        };

        let result = manager.send_transaction(transaction, None).await;
        assert_ok!(result);
        let (manager, _) = result.unwrap();
        let account1_balance = geth.check_balance_in_ethers(&account1);
        assert!(account1_balance == INITIAL_FUNDS - amount1 - 1);
        let account2_balance = geth.check_balance_in_ethers(&account2);
        assert!(account2_balance == amount1);
        manager
    };

    // Sending the second transaction
    let amount2 = 25u64; // in ethers
    {
        let transaction = Transaction {
            priority: Priority::ASAP,
            from: account1.parse().unwrap(),
            to: account2.parse().unwrap(),
            value: Value::Number(ethers::utils::parse_ether(amount2).unwrap()),
            confirmations: 1,
        };

        let result = manager.send_transaction(transaction, None).await;
        assert_ok!(result);
        let (_, _) = result.unwrap();
        let account1_balance = geth.check_balance_in_ethers(&account1);
        assert!(account1_balance == INITIAL_FUNDS - amount1 - amount2 - 1);
        let account2_balance = geth.check_balance_in_ethers(&account2);
        assert!(account2_balance == amount1 + amount2);
    }
}

#[tokio::test]
#[serial]
async fn test_manager_new() {
    setup_tracing();
    let chain_id = U64::from(1u64);

    let transaction = Transaction {
        priority: Priority::Normal,
        from: HASH1.parse().unwrap(),
        to: HASH2.parse().unwrap(),
        value: Value::Number(U256::from(5u64)),
        confirmations: 1,
    };

    // Instantiating a new transaction manager that has no pending transactions.
    {
        let (middleware, gas_oracle, mut db) = setup_dependencies();
        db.get_state_output = Some(None);
        let result = Manager::new(
            middleware,
            gas_oracle,
            db,
            chain_id,
            Configuration::default(),
        )
        .await;
        assert_ok!(result);
        let (_, transaction_receipt) = result.unwrap();
        assert_eq!(transaction_receipt, None);
    }

    // Trying to instantiate new transaction manager without being able to
    // check if there is a transaction pending.
    {
        let (middleware, gas_oracle, mut db) = setup_dependencies();
        db.get_state_output = None;
        let result = Manager::new(
            middleware,
            gas_oracle,
            db,
            chain_id,
            Configuration::default(),
        )
        .await;
        let expected_err: MockManagerError =
            ManagerError::Database(DatabaseError::GetState);
        assert_err!(result, expected_err);
    }

    // Instantiating a new transaction manager that has one pending transaction.
    // The pending transaction's hash is transaction_hash[0].
    {
        let (mut middleware, gas_oracle, mut db) = setup_dependencies();
        middleware.get_block_number = vec![1];
        middleware.get_transaction_receipt = vec![true];
        db.get_state_output = Some(Some(State {
            nonce: Some(U256::from(1u64)),
            transaction: transaction.clone(),
            pending_transactions: vec![TRANSACTION_HASH1.parse().unwrap()],
        }));
        db.clear_state_output = Some(());
        let result = Manager::new(
            middleware,
            gas_oracle,
            db,
            chain_id,
            Configuration {
                transaction_mining_time: Duration::ZERO,
                block_time: Duration::ZERO,
                time: Time,
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
        db.get_state_output = Some(Some(State {
            nonce: Some(U256::from(1u64)),
            transaction: transaction.clone(),
            pending_transactions: vec![TRANSACTION_HASH1.parse().unwrap()],
        }));
        let result = Manager::new(
            middleware,
            gas_oracle,
            db,
            chain_id,
            Configuration {
                transaction_mining_time: Duration::ZERO,
                block_time: Duration::ZERO,
                time: Time,
            },
        )
        .await;
        let expected_err: MockManagerError =
            ManagerError::Database(DatabaseError::ClearState);
        assert_err!(result, expected_err);
    }
}

#[tokio::test]
#[serial]
async fn test_manager_send_transaction_advanced() {
    setup_tracing();

    // Resends the transaction once.
    {
        let result =
            run_send_transaction(1, |mut middleware, gas_oracle, db| {
                middleware.get_block_number = vec![1];
                middleware.get_transaction_receipt = vec![false, true];
                (middleware, gas_oracle, db)
            })
            .await;
        assert_ok!(result);

        assert_eq!(0, MockMiddleware::global().estimate_eip1559_fees_n);
        assert_eq!(1, MockMiddleware::global().get_block_number_n);
        assert_eq!(2, MockMiddleware::global().get_block_n);
        assert_eq!(1, MockMiddleware::global().get_transaction_count_n);
        assert_eq!(2, MockMiddleware::global().estimate_gas_n);
        assert_eq!(2, MockMiddleware::global().sign_transaction_n);
        assert_eq!(2, MockMiddleware::global().send_transaction_n);
        assert_eq!(2, MockMiddleware::global().get_transaction_receipt_n);
    }

    // Resends the transaction twice.
    {
        let result =
            run_send_transaction(1, |mut middleware, gas_oracle, db| {
                middleware.get_block_number = vec![1];
                middleware.get_transaction_receipt =
                    vec![false, false, false, true];
                (middleware, gas_oracle, db)
            })
            .await;
        assert_ok!(result);

        assert_eq!(0, MockMiddleware::global().estimate_eip1559_fees_n);
        assert_eq!(1, MockMiddleware::global().get_block_number_n);
        assert_eq!(3, MockMiddleware::global().get_block_n);
        assert_eq!(1, MockMiddleware::global().get_transaction_count_n);
        assert_eq!(3, MockMiddleware::global().estimate_gas_n);
        assert_eq!(3, MockMiddleware::global().sign_transaction_n);
        assert_eq!(3, MockMiddleware::global().send_transaction_n);
        assert_eq!(4, MockMiddleware::global().get_transaction_receipt_n);
    }
}

#[tokio::test]
#[serial]
async fn test_manager_send_transaction_basic() {
    setup_tracing();

    // Ok (0 confirmations).
    {
        let result = run_send_transaction(0, |middleware, gas_oracle, db| {
            (middleware, gas_oracle, db)
        })
        .await;
        assert_ok!(result);

        assert_eq!(0, MockMiddleware::global().estimate_eip1559_fees_n);
        assert_eq!(1, MockMiddleware::global().get_block_number_n);
        assert_eq!(1, MockMiddleware::global().get_block_n);
        assert_eq!(1, MockMiddleware::global().get_transaction_count_n);
        assert_eq!(1, MockMiddleware::global().estimate_gas_n);
        assert_eq!(1, MockMiddleware::global().sign_transaction_n);
        assert_eq!(1, MockMiddleware::global().send_transaction_n);
        assert_eq!(1, MockMiddleware::global().get_transaction_receipt_n);
    }

    // Ok (1 confirmation).
    {
        let result =
            run_send_transaction(1, |mut middleware, gas_oracle, db| {
                middleware.get_block_number = vec![1];
                middleware.get_transaction_receipt = vec![true];
                (middleware, gas_oracle, db)
            })
            .await;
        assert_ok!(result);

        assert_eq!(0, MockMiddleware::global().estimate_eip1559_fees_n);
        assert_eq!(1, MockMiddleware::global().get_block_number_n);
        assert_eq!(1, MockMiddleware::global().get_block_n);
        assert_eq!(1, MockMiddleware::global().get_transaction_count_n);
        assert_eq!(1, MockMiddleware::global().estimate_gas_n);
        assert_eq!(1, MockMiddleware::global().sign_transaction_n);
        assert_eq!(1, MockMiddleware::global().send_transaction_n);
        assert_eq!(1, MockMiddleware::global().get_transaction_receipt_n);
    }

    // Ok (2 confirmations).
    {
        let result =
            run_send_transaction(2, |mut middleware, gas_oracle, db| {
                middleware.get_block_number = vec![1, 1, 1, 2];
                middleware.get_transaction_receipt = vec![true; 4];
                (middleware, gas_oracle, db)
            })
            .await;
        assert_ok!(result);

        assert_eq!(0, MockMiddleware::global().estimate_eip1559_fees_n);
        assert_eq!(4, MockMiddleware::global().get_block_number_n);
        assert_eq!(1, MockMiddleware::global().get_block_n);
        assert_eq!(1, MockMiddleware::global().get_transaction_count_n);
        assert_eq!(1, MockMiddleware::global().estimate_gas_n);
        assert_eq!(1, MockMiddleware::global().sign_transaction_n);
        assert_eq!(1, MockMiddleware::global().send_transaction_n);
        assert_eq!(4, MockMiddleware::global().get_transaction_receipt_n);
    }

    // Ok (10 confirmations).
    {
        let result =
            run_send_transaction(10, |mut middleware, gas_oracle, db| {
                middleware.get_block_number =
                    vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
                middleware.get_transaction_receipt = vec![true; 10];
                (middleware, gas_oracle, db)
            })
            .await;
        assert_ok!(result);

        assert_eq!(10, MockMiddleware::global().get_block_number_n);
        assert_eq!(1, MockMiddleware::global().get_block_n);
        assert_eq!(1, MockMiddleware::global().get_transaction_count_n);
        assert_eq!(1, MockMiddleware::global().estimate_gas_n);
        assert_eq!(1, MockMiddleware::global().sign_transaction_n);
        assert_eq!(1, MockMiddleware::global().send_transaction_n);
        assert_eq!(10, MockMiddleware::global().get_transaction_receipt_n);
    }
}

#[tokio::test]
#[serial]
async fn test_manager_send_transaction_basic_middleware_errors() {
    setup_tracing();

    // "Middleware::estimate_eip1559_fees" is being tested in the
    // test_manager_send_transaction_basic_gas_oracle_errors function bellow.

    // When "Middleware::get_block" fails.
    {
        let result =
            run_send_transaction(0, |mut middleware, gas_oracle, db| {
                middleware.get_block = None;
                (middleware, gas_oracle, db)
            })
            .await;
        let expected_err: MockManagerError =
            ManagerError::Middleware(MockMiddlewareError::GetBlock);
        assert_err!(result, expected_err);
        assert_eq!(1, MockMiddleware::global().get_block_n)
    }

    // When "Middleware::get_transaction_count" fails.
    {
        let result =
            run_send_transaction(0, |mut middleware, gas_oracle, db| {
                middleware.get_transaction_count = None;
                (middleware, gas_oracle, db)
            })
            .await;
        let expected_err: MockManagerError =
            ManagerError::Middleware(MockMiddlewareError::GetTransactionCount);
        assert_err!(result, expected_err);
        assert_eq!(1, MockMiddleware::global().get_transaction_count_n)
    }

    // When "Middleware::estimate_gas" fails.
    {
        let result =
            run_send_transaction(0, |mut middleware, gas_oracle, db| {
                middleware.estimate_gas = None;
                (middleware, gas_oracle, db)
            })
            .await;
        let expected_err: MockManagerError =
            ManagerError::Middleware(MockMiddlewareError::EstimateGas);
        assert_err!(result, expected_err);
        assert_eq!(1, MockMiddleware::global().estimate_gas_n)
    }

    // When "Middleware::sign_transaction" fails.
    {
        let result =
            run_send_transaction(0, |mut middleware, gas_oracle, db| {
                middleware.sign_transaction = None;
                (middleware, gas_oracle, db)
            })
            .await;
        let expected_err: MockManagerError =
            ManagerError::Middleware(MockMiddlewareError::SignTransaction);
        assert_err!(result, expected_err);
        assert_eq!(1, MockMiddleware::global().sign_transaction_n)
    }

    // When "Middleware::send_transaction" fails.
    {
        let result =
            run_send_transaction(0, |mut middleware, gas_oracle, db| {
                middleware.send_transaction = None;
                (middleware, gas_oracle, db)
            })
            .await;
        let expected_err: MockManagerError =
            ManagerError::Middleware(MockMiddlewareError::SendTransaction);
        assert_err!(result, expected_err);
        assert_eq!(1, MockMiddleware::global().send_transaction_n)
    }

    // When "Middleware::get_transaction_receipt" fails.
    {
        let result =
            run_send_transaction(0, |mut middleware, gas_oracle, db| {
                middleware.get_transaction_receipt = vec![];
                (middleware, gas_oracle, db)
            })
            .await;
        let expected_err: MockManagerError = ManagerError::Middleware(
            MockMiddlewareError::GetTransactionReceipt(1),
        );
        assert_err!(result, expected_err);
        assert_eq!(1, MockMiddleware::global().get_transaction_receipt_n)
    }

    // When "Middleware::get_block_number" fails.
    {
        let result =
            run_send_transaction(0, |mut middleware, gas_oracle, db| {
                middleware.get_block_number = vec![];
                (middleware, gas_oracle, db)
            })
            .await;
        let expected_err: MockManagerError =
            ManagerError::Middleware(MockMiddlewareError::GetBlockNumber);
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
        let result =
            run_send_transaction(0, |mut middleware, mut gas_oracle, db| {
                middleware.estimate_eip1559_fees = Some((300, 50));
                gas_oracle.gas_info_output = None;
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
        let result =
            run_send_transaction(0, |middleware, mut gas_oracle, db| {
                gas_oracle.gas_info_output = None;
                (middleware, gas_oracle, db)
            })
            .await;
        let expected_err: MockManagerError = ManagerError::GasOracle(
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
        let result =
            run_send_transaction(0, |middleware, gas_oracle, mut db| {
                db.set_state_output = None;
                (middleware, gas_oracle, db)
            })
            .await;
        let expected_err: MockManagerError =
            ManagerError::Database(DatabaseError::SetState);
        assert_err!(result, expected_err);
        assert_eq!(1, Database::global().set_state_n);
    }

    // When "Database::clear_state" fails.
    {
        let result =
            run_send_transaction(0, |middleware, gas_oracle, mut db| {
                db.clear_state_output = None;
                (middleware, gas_oracle, db)
            })
            .await;
        let expected_err: MockManagerError =
            ManagerError::Database(DatabaseError::ClearState);
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
        // .with_max_level(tracing::Level::TRACE)
        .event_format(format)
        .try_init();
}

fn setup_dependencies() -> (MockMiddleware, GasOracle, Database) {
    (MockMiddleware::new(), GasOracle::new(), Database::new())
}

async fn setup_manager(
    middleware: MockMiddleware,
    gas_oracle: GasOracle,
    mut db: Database,
) -> Manager<MockMiddleware, GasOracle, Database, Time> {
    db.get_state_output = Some(None);
    let result = Manager::new(
        middleware,
        gas_oracle,
        db,
        U64::from(1), // chain id
        Configuration {
            transaction_mining_time: Duration::ZERO,
            block_time: Duration::ZERO,
            time: Time,
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
    middleware.get_block = Some(());
    middleware.get_block_number = vec![1];
    middleware.get_transaction_count = Some(());
    middleware.get_transaction_receipt = vec![true];
    middleware.send_transaction = Some(());
    middleware.sign_transaction = Some(());
    middleware
}

async fn run_send_transaction(
    confirmations: u32,
    f: fn(
        MockMiddleware,
        GasOracle,
        Database,
    ) -> (MockMiddleware, GasOracle, Database),
) -> Result<TransactionReceipt, MockManagerError> {
    let (mut middleware, mut gas_oracle, mut db) = setup_dependencies();
    middleware = setup_middleware(middleware);
    gas_oracle.gas_info_output = Some(GasInfo {
        gas_price: U256::from_dec_str("3000000000000").unwrap(),
        mining_time: Some(Duration::ZERO),
        block_time: Some(Duration::ZERO),
    });
    db.get_state_output = None;
    db.set_state_output = Some(());
    db.clear_state_output = Some(());
    let (middleware, gas_oracle, db) = f(middleware, gas_oracle, db);

    let manager = setup_manager(middleware, gas_oracle, db).await;
    let transaction = Transaction {
        priority: Priority::Normal,
        from: HASH1.parse().unwrap(),
        to: HASH2.parse().unwrap(),
        value: Value::Number(U256::from(5u64)),
        confirmations,
    };
    manager
        .send_transaction(transaction, Some(Duration::ZERO))
        .await
        .map(|(_, receipt)| receipt)
}
