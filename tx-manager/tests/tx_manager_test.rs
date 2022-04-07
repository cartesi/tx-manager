/*
use ethers::providers::{Http, Provider};
use ethers::types::U256;

use tx_manager::gas_pricer::GasPricer;
use tx_manager::manager::Manager;
use tx_manager::transaction::{Priority, Transaction, Value};

fn ethers(n: u8) -> U256 {
    return (n.to_string() + &"0".repeat(9)).parse().unwrap();
}
async fn test_transaction_manager() {
    let address = "http://127.0.0.1:7545";
    let provider = Provider::<Http>::try_from(address).unwrap();
    let gas_pricer = GasPricer {};
    let mut manager = Manager::new(provider, gas_pricer);

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
}
*/
