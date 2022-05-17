mod mocks;

use anyhow::anyhow;
use ethers::providers::Http;
use ethers::providers::Provider as EthersProvider;
use ethers::types::U256;
use std::fmt::Display;
use std::ptr::addr_of_mut;
use std::time::Duration;

use tx_manager::manager::{Manager, ManagerError, State};
use tx_manager::transaction::{Priority, Transaction, Value};

use mocks::{Data, Database, DatabaseError, GasOracle, Provider};

// TODO: gas : 21000

#[tokio::test]
async fn test_manager_new() {
    let data = Data::new();

    let transaction1 = Transaction {
        priority: Priority::Normal,
        from: data.address[0],
        to: data.address[1],
        value: Value::Number(u256(5)),
        confirmations: 1,
    };

    // Instantiating a new transaction manager that has no pending transactions.
    {
        let (provider, gas_oracle, mut db) = setup_dependencies();
        db.get_state_output = Some(None);
        let result = Manager::new(provider, gas_oracle, db).await;
        assert_ok(&result);
        let (_, transaction_receipt) = result.unwrap();
        assert_eq!(transaction_receipt, None);
    }

    // Trying to instantiate new transaction manager without being able to
    // check if there is a transaction pending.
    {
        let (provider, gas_oracle, mut db) = setup_dependencies();
        db.get_state_output = None;
        let result = Manager::new(provider, gas_oracle, db).await;
        let expected_err =
            ManagerError::GetState(anyhow!(DatabaseError::GetState));
        assert_err(&result, expected_err);
    }

    // Instantiating a new transaction manager that has one pending transaction.
    // The pending transaction's hash is transaction_hash[0].
    let transaction_receipt1 = {
        let (mut provider, gas_oracle, mut db) = setup_dependencies();
        provider.get_block_number = Some(());
        provider.get_transaction_receipt = Some(());
        db.get_state_output = Some(Some(State {
            nonce: Some(u256(1)),
            transaction: transaction1.clone(),
            pending_transactions: vec![data.transaction_hash[0]],
        }));
        db.clear_state_output = Some(());
        let zero_sec = Duration::from_secs(0);
        let result =
            Manager::new_(provider, gas_oracle, db, zero_sec, zero_sec).await;
        assert_ok(&result);
        let (_, transaction_receipt) = result.unwrap();
        transaction_receipt.unwrap()
    };

    // Trying to instantiate a new transaction manager that has one pending
    // transaction without being able to clear the state after the confirmation.
    // The pending transaction's hash is transaction_hash[0].
    {
        let (mut provider, gas_oracle, mut db) = setup_dependencies();
        provider.get_block_number = Some(());
        provider.get_transaction_receipt = Some(());
        db.get_state_output = Some(Some(State {
            nonce: Some(u256(1)),
            transaction: transaction1.clone(),
            pending_transactions: vec![data.transaction_hash[0]],
        }));
        let one_sec = Duration::from_secs(1);
        let result =
            Manager::new_(provider, gas_oracle, db, one_sec, one_sec).await;
        let expected_err = ManagerError::ClearState(
            anyhow!(DatabaseError::ClearState),
            transaction_receipt1,
        );
        assert_err(&result, expected_err);
    }
}

#[tokio::test]
async fn test_manager_send_transaction() {
    let data = Data::new();

    let transaction1 = Transaction {
        priority: Priority::Normal,
        from: data.address[0],
        to: data.address[1],
        value: Value::Number(u256(5)),
        confirmations: 1,
    };

    // Ok.
    unsafe {
        let (mut provider, mut gas_oracle, mut db) = setup_dependencies();
        let (provider_ptr, gas_oracle_ptr, db_ptr) = (
            addr_of_mut!(provider),
            addr_of_mut!(gas_oracle),
            addr_of_mut!(db),
        );
        let mut manager = setup_manager(provider, gas_oracle, db).await;
        (*db_ptr).set_state_output = Some(());
        let result = manager
            .send_transaction(transaction1, Some(Duration::ZERO))
            .await;
        assert_ok(&result);
    }
}

// Auxiliary functions.

fn setup_dependencies() -> (Provider<EthersProvider<Http>>, GasOracle, Database)
{
    Provider::<EthersProvider<Http>>::setup_state();
    (
        Provider::<EthersProvider<Http>>::new(),
        GasOracle::new(),
        Database::new(),
    )
}

async fn setup_manager(
    provider: Provider<EthersProvider<Http>>,
    gas_oracle: GasOracle,
    mut db: Database,
) -> Manager<Provider<EthersProvider<Http>>, GasOracle, Database> {
    db.get_state_output = Some(None);
    let result =
        Manager::new_(provider, gas_oracle, db, Duration::ZERO, Duration::ZERO)
            .await;
    assert_ok(&result);
    let (manager, transaction_receipt) = result.unwrap();
    assert_eq!(transaction_receipt, None);
    manager
}

fn assert_ok<T, E: Display>(output: &Result<T, E>) {
    let _ = output
        .as_ref()
        .map_err(|err| panic!("expected ok, got error: {}", err));
}

fn assert_err<T>(output: &Result<T, ManagerError>, expected: ManagerError) {
    match output {
        Ok(_) => panic!("expected error: {}", expected),
        Err(err) => assert_eq!(err.to_string(), expected.to_string()),
    }
}

fn u256(n: u32) -> U256 {
    U256::from(n)
}
