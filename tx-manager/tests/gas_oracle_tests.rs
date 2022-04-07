use tx_manager::gas_oracle::GasOracle;
use tx_manager::transaction::Priority;

use tracing_subscriber;

#[tokio::test]
async fn test_gas_oracle() {
    tracing_subscriber::fmt::init();

    let gas_oracle = GasOracle {
        api_key: "c5396ceb50a0c347dba8de605f47ffc8e9fd347495b57da6a2d537f78848"
            .to_string(),
    };

    let result = gas_oracle.estimate_eip1559_fees(Priority::Normal).await;
    assert!(result.is_ok());
}
