use ethers::providers::{Http, Provider};
use ethers::types::H160;
use tx_manager::{GasPricer, Tx, TxError, TxManager, TxPriority, TxValue};

#[tokio::test]
async fn test_transaction_manager() {
    let address = "http://127.0.0.1:7545";
    let provider = Provider::<Http>::try_from(address).unwrap();
    let gas_pricer = GasPricer {};
    let mut txm = TxManager::new(provider, gas_pricer);

    let tx = Tx {
        label: "txA",
        priority: TxPriority::Normal,
        from: H160::zero(),
        to: H160::zero(),
        value: TxValue::All,
    };
    let res = txm.send_transaction(tx, 10).await;
    assert!(res.unwrap_err() == TxError::TODO);

    assert_eq!(2 + 2, 4);
}
