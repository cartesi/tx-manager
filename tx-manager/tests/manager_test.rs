mod mocks;

use anyhow::anyhow;
use ethers::core::rand;
use ethers::middleware::signer::SignerMiddleware;
use ethers::providers::{Http, Provider};
use ethers::signers::{LocalWallet, Signer};
use ethers::types::{Address, TransactionReceipt, TxHash, U256, U64};
use ethers::utils::Geth;
use serial_test::serial;
use std::process::Command;
use std::time::Duration;

use tx_manager::database::FileSystemDatabase;
use tx_manager::gas_oracle::{ETHGasStationOracle, GasInfo};
use tx_manager::manager::{Configuration, Manager, ManagerError, State};
use tx_manager::time::DefaultTime;
use tx_manager::transaction::{Priority, Transaction, Value};

use mocks::{
    Database, DatabaseError, GasOracle, GasOracleError, MockMiddleware,
    MockMiddlewareError, Time,
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
async fn test_manager_with_geth() {
    /*
    let port = 8545u16;
    let block_time = 5u64;
    let geth = Geth::new().port(port).block_time(block_time).spawn();

    let url = format!("http://localhost:{}", port).to_string();
    let cmd = Command::new("geth").args(["attach", &url, "--exec"]);

    cmd.arg("eth.accounts");

    drop(geth);
    // let str = "geth attach http://localhost:8545 --exec eth.accounts";
    */

    /*
    Data::setup();
    let transaction = Transaction {
        priority: Priority::Normal,
        from: "0xd631c4a28b6ad5bb5d6b0de6f17a0f13b5fc64f0"
            .parse()
            .unwrap(),
        to: "0x5f68ec5f2bc8ba86a31c4b015b517e165be9b47b"
            .parse()
            .unwrap(),
        value: Value::Number(U256::from(10e18 as u64)), // 10 ethers
        confirmations: 3,
    };

    let port = 8545u16;
    // let block_time = 1u64;
    let url = format!("http://localhost:{}", port).to_string();

    let chain_id = 1337u64;

    let provider = Provider::<Http>::try_from(url).unwrap();
    let unused = &mut rand::thread_rng();
    let signer = LocalWallet::new(unused).with_chain_id(chain_id);
    let provider = SignerMiddleware::new(provider, signer);

    let gas_oracle = ETHGasStationOracle::new("api key");
    let database = FileSystemDatabase::new("./test_database.json");
    let result = Manager::new_(
        provider,
        gas_oracle,
        database,
        Box::new(DefaultTime),
        chain_id.into(),
        Duration::from_secs(2),
        Duration::from_secs(2),
    )
    .await;

    assert_ok!(result);
    let (manager, _) = result.unwrap();

    // let geth = Geth::new().port(port).block_time(block_time).spawn();
    let result = manager.send_transaction(transaction, None).await;
    // drop(geth);
    assert_ok!(result);
    let (_manager, _receipt) = result.unwrap();
    */
}

#[tokio::test]
#[serial]
async fn test_manager_new() {
    Data::setup();
    let chain_id = U64::from(1u64);

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
        let expected_err: ManagerError<MockMiddleware, Database> =
            ManagerError::Database(DatabaseError::GetState);
        assert_err!(result, expected_err);
    }

    // Instantiating a new transaction manager that has one pending transaction.
    // The pending transaction's hash is transaction_hash[0].
    let transaction_receipt = {
        let (mut middleware, gas_oracle, mut db) = setup_dependencies();
        middleware.get_block_number = vec![1];
        middleware.get_transaction_receipt = vec![true];
        db.get_state_output = Some(Some(State {
            nonce: Some(u256(1)),
            transaction: transaction.clone(),
            pending_transactions: vec![Data::get().transaction_hash[0]],
        }));
        db.clear_state_output = Some(());
        let result = Manager::new(
            middleware,
            gas_oracle,
            db,
            chain_id,
            Configuration {
                transaction_mining_interval: Duration::ZERO,
                block_time: Duration::ZERO,
                time: Time,
            },
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
        middleware.get_block_number = vec![1];
        middleware.get_transaction_receipt = vec![true];
        db.get_state_output = Some(Some(State {
            nonce: Some(u256(1)),
            transaction: transaction.clone(),
            pending_transactions: vec![Data::get().transaction_hash[0]],
        }));
        let result = Manager::new(
            middleware,
            gas_oracle,
            db,
            chain_id,
            Configuration {
                transaction_mining_interval: Duration::ZERO,
                block_time: Duration::ZERO,
                time: Time,
            },
        )
        .await;
        let expected_err: ManagerError<MockMiddleware, Database> =
            ManagerError::ClearState(
                DatabaseError::ClearState,
                transaction_receipt,
            );
        assert_err!(result, expected_err);
    }
}

#[tokio::test]
#[serial]
async fn test_manager_send_transaction_advanced() {
    Data::setup();

    // Resends the transaction once.
    {
        let result =
            run_send_transaction(1, |mut middleware, gas_oracle, db| {
                middleware.get_block_number = vec![0, 1, 2];
                middleware.get_transaction_receipt = vec![false, true];
                (middleware, gas_oracle, db)
            })
            .await;
        assert_ok!(result);

        assert_eq!(0, MockMiddleware::global().estimate_eip1559_fees_n);
        assert_eq!(3, MockMiddleware::global().get_block_number_n);
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
                middleware.get_block_number = vec![0, 1, 2, 3];
                middleware.get_transaction_receipt =
                    vec![false, false, false, true];
                (middleware, gas_oracle, db)
            })
            .await;
        assert_ok!(result);

        assert_eq!(0, MockMiddleware::global().estimate_eip1559_fees_n);
        assert_eq!(4, MockMiddleware::global().get_block_number_n);
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
    Data::setup();

    // Ok (1 confirmation).
    {
        let result = run_send_transaction(1, |middleware, gas_oracle, db| {
            (middleware, gas_oracle, db)
        })
        .await;
        assert_ok!(result);

        assert_eq!(0, MockMiddleware::global().estimate_eip1559_fees_n);
        assert_eq!(2, MockMiddleware::global().get_block_number_n);
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
                middleware.get_block_number = vec![0, 1, 1, 1, 2];
                middleware.get_transaction_receipt = vec![true; 4];
                (middleware, gas_oracle, db)
            })
            .await;
        assert_ok!(result);

        assert_eq!(0, MockMiddleware::global().estimate_eip1559_fees_n);
        assert_eq!(5, MockMiddleware::global().get_block_number_n);
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
                    vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
                middleware.get_transaction_receipt = vec![true; 10];
                (middleware, gas_oracle, db)
            })
            .await;
        assert_ok!(result);

        assert_eq!(11, MockMiddleware::global().get_block_number_n);
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
    Data::setup();

    // "Middleware::estimate_eip1559_fees" is being tested in the
    // test_manager_send_transaction_basic_gas_oracle_errors function bellow.

    // When "Middleware::get_block_number" fails
    // inside "Manager::send_transaction_".
    {
        let result =
            run_send_transaction(1, |mut middleware, gas_oracle, db| {
                middleware.get_block_number = vec![];
                (middleware, gas_oracle, db)
            })
            .await;
        assert_err!(
            result,
            ManagerError::<MockMiddleware, Database>::Middleware(
                MockMiddlewareError::GetBlockNumber,
            )
        );
        assert_eq!(1, MockMiddleware::global().get_block_number_n)
    }

    // When "Middleware::get_block" fails.
    {
        let result =
            run_send_transaction(1, |mut middleware, gas_oracle, db| {
                middleware.get_block = None;
                (middleware, gas_oracle, db)
            })
            .await;
        assert_err!(
            result,
            ManagerError::<MockMiddleware, Database>::Middleware(
                MockMiddlewareError::GetBlock,
            )
        );
        assert_eq!(1, MockMiddleware::global().get_block_n)
    }

    // When "Middleware::get_transaction_count" fails.
    {
        let result =
            run_send_transaction(1, |mut middleware, gas_oracle, db| {
                middleware.get_transaction_count = None;
                (middleware, gas_oracle, db)
            })
            .await;
        assert_err!(
            result,
            ManagerError::<MockMiddleware, Database>::Middleware(
                MockMiddlewareError::GetTransactionCount,
            )
        );
        assert_eq!(1, MockMiddleware::global().get_transaction_count_n)
    }

    // When "Middleware::estimate_gas" fails.
    {
        let result =
            run_send_transaction(1, |mut middleware, gas_oracle, db| {
                middleware.estimate_gas = None;
                (middleware, gas_oracle, db)
            })
            .await;
        assert_err!(
            result,
            ManagerError::<MockMiddleware, Database>::Middleware(
                MockMiddlewareError::EstimateGas,
            )
        );
        assert_eq!(1, MockMiddleware::global().estimate_gas_n)
    }

    // When "Middleware::sign_transaction" fails.
    {
        let result =
            run_send_transaction(1, |mut middleware, gas_oracle, db| {
                middleware.sign_transaction = None;
                (middleware, gas_oracle, db)
            })
            .await;
        assert_err!(
            result,
            ManagerError::<MockMiddleware, Database>::Middleware(
                MockMiddlewareError::SignTransaction,
            )
        );
        assert_eq!(1, MockMiddleware::global().sign_transaction_n)
    }

    // When "Middleware::send_transaction" fails.
    {
        let result =
            run_send_transaction(1, |mut middleware, gas_oracle, db| {
                middleware.send_transaction = None;
                (middleware, gas_oracle, db)
            })
            .await;
        assert_err!(
            result,
            ManagerError::<MockMiddleware, Database>::Middleware(
                MockMiddlewareError::SendTransaction,
            )
        );
        assert_eq!(1, MockMiddleware::global().send_transaction_n)
    }

    // When "Middleware::get_transaction_receipt" fails.
    {
        let result =
            run_send_transaction(1, |mut middleware, gas_oracle, db| {
                middleware.get_transaction_receipt = vec![];
                (middleware, gas_oracle, db)
            })
            .await;
        assert_err!(
            result,
            ManagerError::<MockMiddleware, Database>::Middleware(
                MockMiddlewareError::GetTransactionReceipt(1),
            )
        );
        assert_eq!(1, MockMiddleware::global().get_transaction_receipt_n)
    }

    // When "Middleware::get_block_number"
    // fails inside "Manager::confirm_transaction".
    {
        let result =
            run_send_transaction(1, |mut middleware, gas_oracle, db| {
                middleware.get_block_number = vec![0];
                (middleware, gas_oracle, db)
            })
            .await;
        assert_err!(
            result,
            ManagerError::<MockMiddleware, Database>::Middleware(
                MockMiddlewareError::GetBlockNumber,
            )
        );
        assert_eq!(2, MockMiddleware::global().get_block_number_n)
    }
}

#[tokio::test]
#[serial]
async fn test_manager_send_transaction_basic_gas_oracle_errors() {
    Data::setup();

    // When only "GasOracle::gas_info" fails.
    {
        let result =
            run_send_transaction(1, |mut middleware, mut gas_oracle, db| {
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
            run_send_transaction(1, |middleware, mut gas_oracle, db| {
                gas_oracle.gas_info_output = None;
                (middleware, gas_oracle, db)
            })
            .await;
        assert_err!(
            result,
            ManagerError::<MockMiddleware, Database>::GasOracle(
                anyhow!(GasOracleError::GasInfo,),
                MockMiddlewareError::EstimateEIP1559Fees
            )
        );
        assert_eq!(1, GasOracle::global().gas_info_n);
        assert_eq!(1, MockMiddleware::global().estimate_eip1559_fees_n);
    }
}

#[tokio::test]
#[serial]
async fn test_manager_send_transaction_basic_database_errors() {
    Data::setup();

    // When "Database::set_state" fails.
    {
        let result =
            run_send_transaction(1, |middleware, gas_oracle, mut db| {
                db.set_state_output = None;
                (middleware, gas_oracle, db)
            })
            .await;
        assert_err!(
            result,
            ManagerError::<MockMiddleware, Database>::Database(
                DatabaseError::SetState
            )
        );
        assert_eq!(1, Database::global().set_state_n);
    }

    // When "Database::clear_state" fails.
    {
        let result =
            run_send_transaction(1, |middleware, gas_oracle, mut db| {
                db.clear_state_output = None;
                (middleware, gas_oracle, db)
            })
            .await;
        assert!(result.is_err());
        match result.err().unwrap() {
            ManagerError::ClearState(err, _) => {
                assert_eq!(
                    err.to_string(),
                    DatabaseError::ClearState.to_string()
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
) -> Manager<MockMiddleware, GasOracle, Database, Time> {
    db.get_state_output = Some(None);
    let result = Manager::new(
        middleware,
        gas_oracle,
        db,
        U64::from(1), // chain id
        Configuration {
            transaction_mining_interval: Duration::ZERO,
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
    middleware.estimate_gas = Some(u256(21000));
    middleware.get_block = Some(());
    middleware.get_block_number = vec![0, 1];
    middleware.get_transaction_count = Some(());
    middleware.get_transaction_receipt = vec![true];
    middleware.send_transaction = Some(());
    middleware.sign_transaction = Some(());
    middleware
}

fn u256(n: u32) -> U256 {
    U256::from(n)
}

async fn run_send_transaction(
    confirmations: u32,
    f: fn(
        MockMiddleware,
        GasOracle,
        Database,
    ) -> (MockMiddleware, GasOracle, Database),
) -> Result<TransactionReceipt, ManagerError<MockMiddleware, Database>> {
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
        from: Data::get().address[0],
        to: Data::get().address[1],
        value: Value::Number(u256(5)),
        confirmations,
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
        let _ = tracing_subscriber::fmt().event_format(format).try_init();

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
