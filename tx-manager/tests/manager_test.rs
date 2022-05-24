mod mocks;

use anyhow::anyhow;
use ethers::types::{U256, U64};
use serial_test::serial;
use std::time::Duration;

use tx_manager::gas_oracle::GasInfo;
use tx_manager::manager::{Manager, ManagerError, State};
use tx_manager::transaction::{Priority, Transaction, Value};

use mocks::{
    mock_state, Data, Database, DatabaseError, GasOracle, MockMiddleware,
    MockMiddlewareError,
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
    let data = Data::instance();
    let chain_id = U64::from(1);

    let transaction = Transaction {
        priority: Priority::Normal,
        from: data.address[0],
        to: data.address[1],
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
            pending_transactions: vec![data.transaction_hash[0]],
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
            pending_transactions: vec![data.transaction_hash[0]],
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
    todo!()
}

#[tokio::test]
#[serial]
async fn test_manager_send_transaction_basic() {
    let data = Data::instance();

    let transaction = Transaction {
        priority: Priority::Normal,
        from: data.address[0],
        to: data.address[1],
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

        let mut manager = setup_manager(middleware, gas_oracle, db).await;
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

        let mut manager = setup_manager(middleware, gas_oracle, db).await;
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
    let data = Data::instance();

    async fn run_test(
        data: &Data,
        err: MockMiddlewareError,
        f: fn(MockMiddleware) -> MockMiddleware,
    ) {
        let (mut middleware, mut gas_oracle, mut db) = setup_dependencies();
        middleware = setup_middleware(middleware);
        middleware = f(middleware);
        gas_oracle.gas_info_output = Some(GasInfo {
            gas_price: 300,
            mining_time: Some(Duration::ZERO),
            block_time: Some(Duration::ZERO),
        });
        db.set_state_output = Some(());
        db.clear_state_output = Some(());

        let mut manager = setup_manager(middleware, gas_oracle, db).await;
        let transaction = Transaction {
            priority: Priority::Normal,
            from: data.address[0],
            to: data.address[1],
            value: Value::Number(u256(5)),
            confirmations: 1,
        };
        let result = manager
            .send_transaction(transaction, Some(Duration::ZERO))
            .await;
        assert_err!(result, ManagerError::<MockMiddleware>::Middleware(err));
    }

    // Error when "get_block_number" fails inside "Manager::send_transaction_".
    {
        run_test(
            &data,
            MockMiddlewareError::GetBlockNumber,
            |mut middleware| {
                middleware.get_block_number = vec![None, Some(1)];
                middleware
            },
        )
        .await;

        unsafe {
            assert_eq!(1, mock_state.get_block_number_n);
            assert_eq!(0, mock_state.get_block_n);
            assert_eq!(0, mock_state.get_transaction_count_n);
            assert_eq!(0, mock_state.estimate_gas_n);
            assert_eq!(0, mock_state.sign_transaction_n);
            assert_eq!(0, mock_state.send_transaction_n);
            assert_eq!(0, mock_state.get_transaction_receipt_n);
        }
    }

    // Error when "get_block" fails.
    {
        run_test(&data, MockMiddlewareError::GetBlock, |mut middleware| {
            middleware.get_block = None;
            middleware
        })
        .await;

        unsafe {
            assert_eq!(1, mock_state.get_block_number_n);
            assert_eq!(1, mock_state.get_block_n);
            assert_eq!(0, mock_state.get_transaction_count_n);
            assert_eq!(0, mock_state.estimate_gas_n);
            assert_eq!(0, mock_state.sign_transaction_n);
            assert_eq!(0, mock_state.send_transaction_n);
            assert_eq!(0, mock_state.get_transaction_receipt_n);
        }
    }

    // Error when "get_transaction_count" fails.
    {
        run_test(
            &data,
            MockMiddlewareError::GetTransactionCount,
            |mut middleware| {
                middleware.get_transaction_count = None;
                middleware
            },
        )
        .await;

        unsafe {
            assert_eq!(1, mock_state.get_block_number_n);
            assert_eq!(1, mock_state.get_block_n);
            assert_eq!(1, mock_state.get_transaction_count_n);
            assert_eq!(0, mock_state.estimate_gas_n);
            assert_eq!(0, mock_state.sign_transaction_n);
            assert_eq!(0, mock_state.send_transaction_n);
            assert_eq!(0, mock_state.get_transaction_receipt_n);
        }
    }

    // Error when "estimate_gas" fails.
    {
        run_test(&data, MockMiddlewareError::EstimateGas, |mut middleware| {
            middleware.estimate_gas = None;
            middleware
        })
        .await;

        unsafe {
            assert_eq!(1, mock_state.get_block_number_n);
            assert_eq!(1, mock_state.get_block_n);
            assert_eq!(1, mock_state.get_transaction_count_n);
            assert_eq!(1, mock_state.estimate_gas_n);
            assert_eq!(0, mock_state.sign_transaction_n);
            assert_eq!(0, mock_state.send_transaction_n);
            assert_eq!(0, mock_state.get_transaction_receipt_n);
        }
    }

    // Error when "sign_transaction" fails.
    {
        run_test(
            &data,
            MockMiddlewareError::SignTransaction,
            |mut middleware| {
                middleware.sign_transaction = None;
                middleware
            },
        )
        .await;

        unsafe {
            assert_eq!(1, mock_state.get_block_number_n);
            assert_eq!(1, mock_state.get_block_n);
            assert_eq!(1, mock_state.get_transaction_count_n);
            assert_eq!(1, mock_state.estimate_gas_n);
            assert_eq!(1, mock_state.sign_transaction_n);
            assert_eq!(0, mock_state.send_transaction_n);
            assert_eq!(0, mock_state.get_transaction_receipt_n);
        }
    }

    // Error when "send_transaction" fails.
    {
        run_test(
            &data,
            MockMiddlewareError::SendTransaction,
            |mut middleware| {
                middleware.send_transaction = None;
                middleware
            },
        )
        .await;

        unsafe {
            assert_eq!(1, mock_state.get_block_number_n);
            assert_eq!(1, mock_state.get_block_n);
            assert_eq!(1, mock_state.get_transaction_count_n);
            assert_eq!(1, mock_state.estimate_gas_n);
            assert_eq!(1, mock_state.sign_transaction_n);
            assert_eq!(1, mock_state.send_transaction_n);
            assert_eq!(0, mock_state.get_transaction_receipt_n);
        }
    }

    // Error when "get_transaction_receipt" fails.
    {
        run_test(
            &data,
            MockMiddlewareError::GetTransactionReceipt,
            |mut middleware| {
                middleware.get_transaction_receipt = None;
                middleware
            },
        )
        .await;

        unsafe {
            assert_eq!(1, mock_state.get_block_number_n);
            assert_eq!(1, mock_state.get_block_n);
            assert_eq!(1, mock_state.get_transaction_count_n);
            assert_eq!(1, mock_state.estimate_gas_n);
            assert_eq!(1, mock_state.sign_transaction_n);
            assert_eq!(1, mock_state.send_transaction_n);
            assert_eq!(1, mock_state.get_transaction_receipt_n);
        }
    }

    // Error when "get_block_number" fails inside
    // "Manager::confirm_transaction".
    {
        run_test(
            &data,
            MockMiddlewareError::GetBlockNumber,
            |mut middleware| {
                middleware.get_block_number = vec![Some(0), None];
                middleware
            },
        )
        .await;

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
}

#[tokio::test]
#[serial]
async fn test_manager_send_transaction_basic_gas_oracle_errors() {
    todo!()
}

#[tokio::test]
#[serial]
async fn test_manager_send_transaction_basic_database_errors() {
    todo!()
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
