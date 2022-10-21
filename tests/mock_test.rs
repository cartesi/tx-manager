use ethers::types::{TransactionReceipt, U256};
use serial_test::serial;
use std::time::Duration;

use tx_manager::{
    gas_oracle::{EIP1559GasInfo, GasInfo, GasOracle, GasOracleInfo},
    manager::{Configuration, Manager},
    transaction::{PersistentState, Priority, StaticTxData, SubmittedTxs, Transaction, Value},
    Chain,
};

use utilities::{
    assert_err, assert_ok,
    mocks::{
        database::{DatabaseStateError, MockDatabase},
        gas_oracle::{IncrementingGasOracle, MockGasOracle, MockGasOracleError},
        middleware::{MockMiddleware, MockMiddlewareError},
        time::MockTime,
    },
    Account,
};

type MockManagerError = tx_manager::Error<MockMiddleware, MockGasOracle, MockDatabase>;
type MockManagerError2<GO> = tx_manager::Error<MockMiddleware, GO, MockDatabase>;

const CHAIN: Chain = Chain {
    id: 1337,
    is_legacy: false,
};

#[tokio::test]
#[serial]
async fn test_manager_new() {
    utilities::setup_tracing();

    let account1 = Account::random();
    let account2 = Account::random();

    let transaction = Transaction {
        from: account1.into(),
        to: account2.into(),
        value: Value::Number(U256::from(5u64)),
        call_data: None,
    };

    // Instantiating a new transaction manager that has no pending transactions.
    {
        let (middleware, gas_oracle, mut db) = setup_dependencies();
        db.get_state_output = Some(None);
        let result =
            Manager::new(middleware, gas_oracle, db, CHAIN, Configuration::default()).await;
        assert_ok!(result);
        let (_, transaction_receipt) = result.unwrap();
        assert_eq!(transaction_receipt, None);
    }

    // Trying to instantiate new transaction manager without being able to check if
    // there is a transaction pending.
    {
        let (middleware, gas_oracle, mut db) = setup_dependencies();
        db.get_state_output = None;
        let result =
            Manager::new(middleware, gas_oracle, db, CHAIN, Configuration::default()).await;
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
            gas_oracle,
            db,
            CHAIN,
            Configuration {
                transaction_mining_time: Duration::ZERO,
                block_time: Duration::ZERO,
                time: MockTime,
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
            gas_oracle,
            db,
            CHAIN,
            Configuration {
                transaction_mining_time: Duration::ZERO,
                block_time: Duration::ZERO,
                time: MockTime,
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
    utilities::setup_tracing();

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
    utilities::setup_tracing();

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
    utilities::setup_tracing();

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
    utilities::setup_tracing();

    // When only "GasOracle::gas_info" fails.
    {
        let result = run_send_transaction(0, |mut middleware, mut gas_oracle, db| {
            middleware.estimate_eip1559_fees = Some((300, 50));
            gas_oracle.gas_oracle_info_output = None;
            (middleware, gas_oracle, db)
        })
        .await;
        assert_ok!(result);
        assert_eq!(1, MockGasOracle::global().gas_info_n);
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
            MockGasOracleError::GasInfo,
            MockMiddlewareError::EstimateEIP1559Fees,
        );
        assert_err!(result, expected_err);
        assert_eq!(1, MockGasOracle::global().gas_info_n);
        assert_eq!(1, MockMiddleware::global().estimate_eip1559_fees_n);
    }
}

#[tokio::test]
#[serial]
async fn test_manager_send_transaction_basic_database_errors() {
    utilities::setup_tracing();

    // When "Database::set_state" fails.
    {
        let result = run_send_transaction(0, |middleware, gas_oracle, mut db| {
            db.set_state_output = None;
            (middleware, gas_oracle, db)
        })
        .await;
        let expected_err: MockManagerError = tx_manager::Error::Database(DatabaseStateError::Set);
        assert_err!(result, expected_err);
        assert_eq!(1, MockDatabase::global().set_state_n);
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
        assert_eq!(1, MockDatabase::global().clear_state_n);
    }
}

// ------------------------------------------------------------------------------------------------
// Auxiliary
// ------------------------------------------------------------------------------------------------

fn setup_dependencies() -> (MockMiddleware, MockGasOracle, MockDatabase) {
    (
        MockMiddleware::new(),
        MockGasOracle::new(),
        MockDatabase::new(),
    )
}

const HASH1: &str = "0xba763b97851b653aaaf631723bab41a500f03b29";
const HASH2: &str = "0x29e425df042e83e4ddb3ee3348d6d745c58fce8f";

const TRANSACTION_HASH1: &str =
    "0x2b34df791cc4eb898f6d4437713e946f216cac6a3921b2899db919abe26739b2";

async fn setup_manager<GO: tx_manager::gas_oracle::GasOracle>(
    middleware: MockMiddleware,
    gas_oracle: GO,
    mut db: MockDatabase,
) -> Manager<MockMiddleware, GO, MockDatabase, MockTime>
where
    GO: Send + Sync,
{
    db.get_state_output = Some(None);
    let result = Manager::new(
        middleware,
        gas_oracle,
        db,
        CHAIN,
        Configuration {
            transaction_mining_time: Duration::ZERO,
            block_time: Duration::ZERO,
            time: MockTime,
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
    f: fn(
        MockMiddleware,
        MockGasOracle,
        MockDatabase,
    ) -> (MockMiddleware, MockGasOracle, MockDatabase),
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
async fn run_send_transaction2<GO: GasOracle>(
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
