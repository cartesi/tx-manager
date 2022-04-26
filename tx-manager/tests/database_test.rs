use ethers::types::{H160, H256};
use std::fs::File;
use std::path::Path;

use tx_manager::database::{new_file_system_database, Database};
use tx_manager::manager::State;
use tx_manager::transaction::{Priority, Transaction, Value};

#[allow(non_snake_case)]
fn to_H160(n: u64) -> H160 {
    return H160::from_low_u64_ne(n);
}

#[allow(non_snake_case)]
fn to_H256(n: u64) -> H256 {
    return H256::from_low_u64_ne(n);
}

#[tokio::test]
async fn test_database_set_state() {
    // setup
    let path_str = "./database.json";
    let path = Path::new(path_str);
    let database = new_file_system_database(path_str);
    let _ = database.clear_state().await.is_ok();

    // ok => set state over empty state
    let state1 = State {
        nonce: Some(1.into()),
        transaction: Transaction {
            priority: Priority::Normal,
            from: to_H160(1),
            to: to_H160(2),
            value: Value::Number(5000.into()),
            confirmations: 0,
        },
        pending_transactions: vec![],
    };
    assert!(!path.is_file());
    let result = database.set_state(&state1).await;
    assert!(result.is_ok());
    assert!(path.is_file());

    // ok => set state over preexisting state
    let state2 = State {
        nonce: Some(2.into()),
        transaction: Transaction {
            priority: Priority::High,
            from: to_H160(5),
            to: to_H160(6),
            value: Value::Number(3000.into()),
            confirmations: 5,
        },
        pending_transactions: vec![to_H256(1400), to_H256(1500)],
    };
    assert!(path.is_file());
    let result = database.set_state(&state2).await;
    assert!(result.is_ok());
    assert!(path.is_file());

    // teardown
    assert!(database.clear_state().await.is_ok());
}

#[tokio::test]
async fn test_database_get_state() {
    todo!();
}

#[tokio::test]
async fn test_database_clear_state() {
    // setup
    let path_str = "./should_be_removed.json";
    let path = Path::new(path_str);
    assert!(File::create(path_str).is_ok());

    // ok => clearing the state
    assert!(path.is_file());
    let result = new_file_system_database(path_str).clear_state().await;
    assert!(result.is_ok());
    assert!(!path.is_file());

    // error => cannot clear an empty state
    assert!(!path.is_file());
    let result = new_file_system_database(path_str).clear_state().await;
    assert!(result.is_err(), "{:?}", result.err());
    assert!(!path.is_file());
}
