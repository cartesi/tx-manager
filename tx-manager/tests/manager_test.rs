mod mocks;

use anyhow::anyhow;
use ethers::types::{Address, TransactionReceipt, TxHash, U256, U64};
use serial_test::serial;
use std::time::Duration;

use tx_manager::gas_oracle::GasInfo;
use tx_manager::manager::{Manager, ManagerError, State};
use tx_manager::transaction::{Priority, Transaction, Value};

use mocks::{
    mock_state, Database, DatabaseError, GasOracle, GasOracleError,
    MockMiddleware, MockMiddlewareError,
};

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

#[tokio::test]
#[serial]
async fn test_manager_new() {
    Data::setup();
    let chain_id = U64::from(1);

    let transaction = Transaction {
        priority: Priority::Normal,
        from: Data::get().address[0],
        to: Data::get().address[1],
        value: Value::Number(u256(5)),
        confirmations: 1,
    };

    // Instantiating a new transaction manager that has no pending transactions.
    {
        let (middleware, gas_oracle, mut db) = setup_dependencies();
        db.get_state_output = Some(None);
        let result = Manager::new(middleware, gas_oracle, db, chain_id).await;
        assert_ok!(result);
        let (_, transaction_receipt) = result.unwrap();
        assert_eq!(transaction_receipt, None);
    }

    // Trying to instantiate new transaction manager without being able to
    // check if there is a transaction pending.
    {
        let (middleware, gas_oracle, mut db) = setup_dependencies();
        db.get_state_output = None;
        let result = Manager::new(middleware, gas_oracle, db, chain_id).await;
        let expected_err: ManagerError<MockMiddleware> =
            ManagerError::GetState(anyhow!(DatabaseError::GetState));
        assert_err!(result, expected_err);
    }

    // Instantiating a new transaction manager that has one pending transaction.
    // The pending transaction's hash is transaction_hash[0].
    let transaction_receipt = {
        let (mut middleware, gas_oracle, mut db) = setup_dependencies();
        middleware.get_block_number = vec![Some(1)];
        middleware.get_transaction_receipt = Some(());
        db.get_state_output = Some(Some(State {
            nonce: Some(u256(1)),
            transaction: transaction.clone(),
            pending_transactions: vec![Data::get().transaction_hash[0]],
        }));
        db.clear_state_output = Some(());
        let zero_sec = Duration::from_secs(0);
        let result = Manager::new_(
            middleware, gas_oracle, db, chain_id, zero_sec, zero_sec,
        )
        .await;
        assert_ok!(result);
        let (_, transaction_receipt) = result.unwrap();
        assert!(transaction_receipt.is_some());
        transaction_receipt.unwrap()
    };

    // Trying to instantiate a new transaction manager that has one pending
    // transaction without being able to clear the state after the confirmation.
    // The pending transaction's hash is transaction_hash[0].
    {
        let (mut middleware, gas_oracle, mut db) = setup_dependencies();
        middleware.get_block_number = vec![Some(1)];
        middleware.get_transaction_receipt = Some(());
        db.get_state_output = Some(Some(State {
            nonce: Some(u256(1)),
            transaction: transaction.clone(),
            pending_transactions: vec![Data::get().transaction_hash[0]],
        }));
        let one_sec = Duration::from_secs(1);
        let result = Manager::new_(
            middleware, gas_oracle, db, chain_id, one_sec, one_sec,
        )
        .await;
        let expected_err: ManagerError<MockMiddleware> =
            ManagerError::ClearState(
                anyhow!(DatabaseError::ClearState),
                transaction_receipt,
            );
        assert_err!(result, expected_err);
    }
}

#[tokio::test]
#[serial]
async fn test_manager_send_transaction_advanced() {
    Data::setup();
    todo!()
}

#[tokio::test]
#[serial]
async fn test_manager_send_transaction_basic() {
    Data::setup();

    let transaction = Transaction {
        priority: Priority::Normal,
        from: Data::get().address[0],
        to: Data::get().address[1],
        value: Value::Number(u256(5)),
        confirmations: 1, // to be set in each test if necessary
    };

    let gas_info = GasInfo {
        gas_price: 300,
        mining_time: Some(Duration::ZERO),
        block_time: Some(Duration::ZERO),
    };

    // Ok (1 confirmation).
    {
        let (mut middleware, mut gas_oracle, mut db) = setup_dependencies();
        middleware = setup_middleware(middleware);
        gas_oracle.gas_info_output = Some(gas_info);
        db.set_state_output = Some(());
        db.clear_state_output = Some(());

        let manager = setup_manager(middleware, gas_oracle, db).await;
        let result = manager
            .send_transaction(transaction.clone(), Some(Duration::ZERO))
            .await;
        assert_ok!(result);

        unsafe {
            assert_eq!(2, mock_state.get_block_number_n);
            assert_eq!(1, mock_state.get_block_n);
            assert_eq!(1, mock_state.get_transaction_count_n);
            assert_eq!(1, mock_state.estimate_gas_n);
            assert_eq!(1, mock_state.sign_transaction_n);
            assert_eq!(1, mock_state.send_transaction_n);
            assert_eq!(1, mock_state.get_transaction_receipt_n);
        }
    }

    // Ok (2 confirmations).
    {
        let mut transaction = transaction.clone();
        transaction.confirmations = 2;

        let (mut middleware, mut gas_oracle, mut db) = setup_dependencies();
        middleware = setup_middleware(middleware);
        middleware.get_block_number =
            vec![Some(0), Some(1), Some(1), Some(1), Some(2)];
        gas_oracle.gas_info_output = Some(gas_info);
        db.set_state_output = Some(());
        db.clear_state_output = Some(());

        let manager = setup_manager(middleware, gas_oracle, db).await;
        let result = manager
            .send_transaction(transaction, Some(Duration::ZERO))
            .await;
        assert_ok!(result);

        unsafe {
            assert_eq!(5, mock_state.get_block_number_n);
            assert_eq!(1, mock_state.get_block_n);
            assert_eq!(1, mock_state.get_transaction_count_n);
            assert_eq!(1, mock_state.estimate_gas_n);
            assert_eq!(1, mock_state.sign_transaction_n);
            assert_eq!(1, mock_state.send_transaction_n);
            assert_eq!(4, mock_state.get_transaction_receipt_n);
        }
    }
}

#[tokio::test]
#[serial]
async fn test_manager_send_transaction_basic_middleware_errors() {
    Data::setup();

    // When "Middleware::get_block_number" fails
    // inside "Manager::send_transaction_".
    {
        let result = run_send_transaction(|mut middleware, gas_oracle, db| {
            middleware.get_block_number = vec![None, Some(1)];
            (middleware, gas_oracle, db)
        })
        .await;
        assert_err!(
            result,
            ManagerError::<MockMiddleware>::Middleware(
                MockMiddlewareError::GetBlockNumber,
            )
        );
        unsafe { assert_eq!(1, mock_state.get_block_number_n) }
    }

    // When "Middleware::get_block" fails.
    {
        let result = run_send_transaction(|mut middleware, gas_oracle, db| {
            middleware.get_block = None;
            (middleware, gas_oracle, db)
        })
        .await;
        assert_err!(
            result,
            ManagerError::<MockMiddleware>::Middleware(
                MockMiddlewareError::GetBlock,
            )
        );
        unsafe { assert_eq!(1, mock_state.get_block_n) }
    }

    // When "Middleware::get_transaction_count" fails.
    {
        let result = run_send_transaction(|mut middleware, gas_oracle, db| {
            middleware.get_transaction_count = None;
            (middleware, gas_oracle, db)
        })
        .await;
        assert_err!(
            result,
            ManagerError::<MockMiddleware>::Middleware(
                MockMiddlewareError::GetTransactionCount,
            )
        );
        unsafe { assert_eq!(1, mock_state.get_transaction_count_n) }
    }

    // When "Middleware::estimate_gas" fails.
    {
        let result = run_send_transaction(|mut middleware, gas_oracle, db| {
            middleware.estimate_gas = None;
            (middleware, gas_oracle, db)
        })
        .await;
        assert_err!(
            result,
            ManagerError::<MockMiddleware>::Middleware(
                MockMiddlewareError::EstimateGas,
            )
        );
        unsafe { assert_eq!(1, mock_state.estimate_gas_n) }
    }

    // When "Middleware::sign_transaction" fails.
    {
        let result = run_send_transaction(|mut middleware, gas_oracle, db| {
            middleware.sign_transaction = None;
            (middleware, gas_oracle, db)
        })
        .await;
        assert_err!(
            result,
            ManagerError::<MockMiddleware>::Middleware(
                MockMiddlewareError::SignTransaction,
            )
        );
        unsafe { assert_eq!(1, mock_state.sign_transaction_n) }
    }

    // When "Middleware::send_transaction" fails.
    {
        let result = run_send_transaction(|mut middleware, gas_oracle, db| {
            middleware.send_transaction = None;
            (middleware, gas_oracle, db)
        })
        .await;
        assert_err!(
            result,
            ManagerError::<MockMiddleware>::Middleware(
                MockMiddlewareError::SendTransaction,
            )
        );
        unsafe { assert_eq!(1, mock_state.send_transaction_n) }
    }

    // When "Middleware::get_transaction_receipt" fails.
    {
        let result = run_send_transaction(|mut middleware, gas_oracle, db| {
            middleware.get_transaction_receipt = None;
            (middleware, gas_oracle, db)
        })
        .await;
        assert_err!(
            result,
            ManagerError::<MockMiddleware>::Middleware(
                MockMiddlewareError::GetTransactionReceipt,
            )
        );
        unsafe { assert_eq!(1, mock_state.get_transaction_receipt_n) }
    }

    // When "Middleware::get_block_number"
    // fails inside "Manager::confirm_transaction".
    {
        let result = run_send_transaction(|mut middleware, gas_oracle, db| {
            middleware.get_block_number = vec![Some(0), None];
            (middleware, gas_oracle, db)
        })
        .await;
        assert_err!(
            result,
            ManagerError::<MockMiddleware>::Middleware(
                MockMiddlewareError::GetBlockNumber,
            )
        );
        unsafe { assert_eq!(2, mock_state.get_block_number_n) }
    }
}

#[tokio::test]
#[serial]
async fn test_manager_send_transaction_basic_gas_oracle_errors() {
    Data::setup();

    // When "GasOracle::gas_info" fails.
    {
        let result = run_send_transaction(|middleware, mut gas_oracle, db| {
            gas_oracle.gas_info_output = None;
            (middleware, gas_oracle, db)
        })
        .await;
        assert_err!(
            result,
            ManagerError::<MockMiddleware>::GasOracle(anyhow!(
                GasOracleError::GasInfo
            ))
        );
        assert_eq!(1, GasOracle::global().gas_info_n);
    }
}

#[tokio::test]
#[serial]
async fn test_manager_send_transaction_basic_database_errors() {
    Data::setup();

    // When "Database::set_state" fails.
    {
        let result = run_send_transaction(|middleware, gas_oracle, mut db| {
            db.set_state_output = None;
            (middleware, gas_oracle, db)
        })
        .await;
        assert_err!(
            result,
            ManagerError::<MockMiddleware>::SetState(anyhow!(
                DatabaseError::SetState
            ))
        );
        assert_eq!(1, Database::global().set_state_n);
    }

    // When "Database::clear_state" fails.
    {
        let result = run_send_transaction(|middleware, gas_oracle, mut db| {
            db.clear_state_output = None;
            (middleware, gas_oracle, db)
        })
        .await;
        assert!(result.is_err());
        match result.err().unwrap() {
            ManagerError::ClearState(err, _) => {
                assert_err!(
                    Result::<(), anyhow::Error>::Err(err),
                    DatabaseError::ClearState
                )
                // TODO: assert_eq!(1, receipt.block_number)
            }
            _ => assert!(false),
        };
        assert_eq!(1, Database::global().clear_state_n);
    }
}

// Auxiliary functions.

fn setup_dependencies() -> (MockMiddleware, GasOracle, Database) {
    (MockMiddleware::new(), GasOracle::new(), Database::new())
}

async fn setup_manager(
    middleware: MockMiddleware,
    gas_oracle: GasOracle,
    mut db: Database,
) -> Manager<MockMiddleware, GasOracle, Database> {
    db.get_state_output = Some(None);
    let result = Manager::new_(
        middleware,
        gas_oracle,
        db,
        U64::from(1),
        Duration::ZERO,
        Duration::ZERO,
    )
    .await;
    assert_ok!(result);
    let (manager, transaction_receipt) = result.unwrap();
    assert!(transaction_receipt.is_none());
    manager
}

fn setup_middleware(mut middleware: MockMiddleware) -> MockMiddleware {
    middleware.estimate_gas = Some(u256(21000));
    middleware.get_block = Some(());
    middleware.get_block_number = vec![Some(0), Some(1)];
    middleware.get_transaction_count = Some(());
    middleware.get_transaction_receipt = Some(());
    middleware.send_transaction = Some(());
    middleware.sign_transaction = Some(());
    middleware
}

fn u256(n: u32) -> U256 {
    U256::from(n)
}

async fn run_send_transaction(
    f: fn(
        MockMiddleware,
        GasOracle,
        Database,
    ) -> (MockMiddleware, GasOracle, Database),
) -> Result<TransactionReceipt, ManagerError<MockMiddleware>> {
    let (mut middleware, mut gas_oracle, mut db) = setup_dependencies();
    middleware = setup_middleware(middleware);
    gas_oracle.gas_info_output = Some(GasInfo {
        gas_price: 300,
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
        from: Data::get().address[0],
        to: Data::get().address[1],
        value: Value::Number(u256(5)),
        confirmations: 1,
    };
    manager
        .send_transaction(transaction, Some(Duration::ZERO))
        .await
        .map(|(_, receipt)| receipt)
}

// Mocked data.

static mut DATA: Data = Data::default();

#[derive(Debug)]
struct Data {
    address: Vec<Address>,
    transaction_hash: Vec<TxHash>,
}

impl Data {
    const fn default() -> Data {
        Data {
            address: Vec::new(),
            transaction_hash: Vec::new(),
        }
    }

    fn get() -> &'static Data {
        unsafe { &DATA }
    }

    fn setup() {
        let address = [
            "0xba763b97851b653aaaf631723bab41a500f03b29",
            "0x29e425df042e83e4ddb3ee3348d6d745c58fce8f",
            "0x905f3bd1bd9cd23be618454e58ab9e4a104909a9",
            "0x7e2d4b75bbf489e691f8a1f7e5f2f1148e15feed",
        ]
        .map(|s| s.parse().unwrap())
        .to_vec();

        let transaction_hash = [
            "0x2b34df791cc4eb898f6d4437713e946f216cac6a3921b2899db919abe26739b2",
            "0x4eb76dd4a6f6d37212f3b26da6a026c30a92700cdf560f81b14bc42c2cffb218",
            "0x08bd64232916289006f3de2c1cad8e5afa6eabcf4efff219721b87ee6f9084ec",
            "0xffff364da9e2b4bca9199197c220ae334174527c12180f9d667005a887ff2fd6",
        ]
        .map(|s| s.parse().unwrap()).to_vec();

        unsafe {
            DATA = Data {
                address,
                transaction_hash,
            }
        }
    }
}
