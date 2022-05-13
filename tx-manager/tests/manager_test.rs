mod mocks;

use anyhow::anyhow;
use ethers::providers::Http;
use ethers::providers::Provider as EthersProvider;
use ethers::types::U256;
use std::time::Duration;

use tx_manager::manager::{Manager, ManagerError, State};
use tx_manager::transaction::{Priority, Transaction, Value};

use mocks::{
    Data, Database, DatabaseError, GasOracle, GasOracleError, Provider,
};

fn assert_ok<T>(output: Result<T, ManagerError>) {
    let _ = output.map_err(|err| panic!("expected ok, got error: {}", err));
}

fn assert_err<T>(output: Result<T, ManagerError>, expected: ManagerError) {
    match output {
        Ok(_) => panic!("expected error: {}", expected),
        Err(err) => assert_eq!(err.to_string(), expected.to_string()),
    }
}

#[tokio::test]
async fn test_manager() {
    let data = Data::new();

    let transaction1 = Transaction {
        priority: Priority::Normal,
        from: data.address[0],
        to: data.address[1],
        value: Value::Number(u256(5)),
        confirmations: 1,
    };

    // gas : 21000

    // Instantiating a new transaction manager that has no pending transactions.
    {
        let (provider, gas_oracle, mut db) = setup_dependencies();
        db.get_state_output = Some(None);
        let manager = Manager::new(provider, gas_oracle, db).await;
        assert_ok(manager);
    }

    // Trying to instantiate new transaction manager without being able to
    // check if there is a transaction pending.
    {
        let (provider, gas_oracle, mut db) = setup_dependencies();
        db.get_state_output = None;
        let manager = Manager::new(provider, gas_oracle, db).await;
        let expected_err =
            ManagerError::GetState(anyhow!(DatabaseError::GetState));
        assert_err(manager, expected_err);
    }

    // Instantiating a new transaction manager that has one pending transaction.
    // The pending transaction's hash is transaction_hash[0].
    {
        let (provider, gas_oracle, mut db) = setup_dependencies();
        db.get_state_output = Some(Some(State {
            nonce: Some(u256(1)),
            transaction: transaction1,
            pending_transactions: vec![data.transaction_hash[0]],
        }));
        // TODO: provider mock state
        let one_sec = Duration::from_secs(1);
        let manager =
            Manager::new_(provider, gas_oracle, db, one_sec, one_sec).await;
        assert_ok(manager);
    }

    /*
        // error with the pending transaction
        unsafe {}
    */
    /*
    let tx = Transaction {
        label: "transaction_1",
        priority: Priority::Normal,
        from: "0x1633A6cc4590Cf7d3CBFC73cF8cD26f48ee6D11D"
            .parse()
            .unwrap(),
        to: "0xCEA70F3EbE1CCf3aaaf44A3a6CE5C257E5E67b24"
            .parse()
            .unwrap(),
        value: Value::Value(ethers(5)),
    };
    let res = manager.send_transaction(tx, 1, None).await;
    assert!(res.is_ok(), "not ok => {:?}", res);
    */
}

fn u256(n: u32) -> U256 {
    U256::from(n)
}

fn setup_dependencies() -> (Provider<EthersProvider<Http>>, GasOracle, Database)
{
    (
        Provider::<EthersProvider<Http>>::new(),
        GasOracle::new(),
        Database::new(),
    )
}
