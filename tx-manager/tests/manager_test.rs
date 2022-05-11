mod mocks;

use ethers::providers::Http;
use ethers::providers::Provider as EthersProvider;
use ethers::types::{H256, U256};
use std::ptr;
use std::time::Duration;

use tx_manager::manager::{Manager, State};
use tx_manager::transaction::{Priority, Transaction, Value};

use mocks::{Database, DatabaseError, GasOracle, GasOracleError, Provider};

#[tokio::test]
async fn test_manager() {
    let transaction1 = Transaction {
        priority: Priority::Normal,
        from: "0x1633A6cc4590Cf7d3CBFC73cF8cD26f48ee6D11D"
            .parse()
            .unwrap(),
        to: "0xCEA70F3EbE1CCf3aaaf44A3a6CE5C257E5E67b24"
            .parse()
            .unwrap(),
        value: Value::Number(u256(5)),
        confirmations: 1,
    };

    // no pending transactions
    unsafe {
        let (provider, gas_oracle, mut db) = setup();
        let db_ptr = ptr::addr_of_mut!(db);
        (*db_ptr).get_state = (true, None);
        let manager = Manager::new(provider, gas_oracle, db).await;
        assert!(manager.is_ok());
    }

    // has a pending transacton
    unsafe {
        let (provider, gas_oracle, mut db) = setup();
        let state = State {
            nonce: Some(u256(5)),
            transaction: transaction1,
            pending_transactions: vec![H256::random()],
        };
        let db_ptr = ptr::addr_of_mut!(db);
        (*db_ptr).get_state = (true, Some(state));
        let one_sec = Duration::from_secs(1);
        let manager =
            Manager::new_(provider, gas_oracle, db, one_sec, one_sec).await;
        assert!(manager.is_ok());
    }

    // error with the pending transaction
    unsafe {}

    /*


    pub struct State {
        pub nonce: Option<U256>,
        pub transaction: Transaction,
        pub pending_transactions: Vec<H256>, // hashes
    }



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

fn setup() -> (Provider<EthersProvider<Http>>, GasOracle, Database) {
    (
        Provider::<EthersProvider<Http>>::new(),
        GasOracle::new(),
        Database::new(),
    )
}
