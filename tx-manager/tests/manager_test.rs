mod mocks;

use tx_manager::manager::Manager;

use mocks::{DatabaseOutput, GasOracleOutput};

#[tokio::test]
async fn test_manager() {
    let provider = mocks::Provider {};
    let gas_oracle = mocks::GasOracle {
        output: GasOracleOutput::Unreachable,
    };
    let db = mocks::Database {
        output: DatabaseOutput::Unreachable,
    };
    let mut manager = Manager::new(provider, gas_oracle, db);
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
