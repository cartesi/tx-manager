use tx_manager::gas_oracle::{new_eth_gas_station_oracle, GasOracle};
use tx_manager::transaction::Priority;

use tracing_subscriber;

#[tokio::test]
async fn test_eth_gas_station_oracle() {
    // setup
    tracing_subscriber::fmt::init();
    let gas_oracle = new_eth_gas_station_oracle(
        "c5396ceb50a0c347dba8de605f47ffc8e9fd347495b57da6a2d537f78848",
    );
    let invalid_gas_oracle1 = new_eth_gas_station_oracle("invalid");
    let invalid_gas_oracle2 = new_eth_gas_station_oracle("");

    // ok => priority low
    let result = gas_oracle.gas_info(Priority::Low).await;
    assert!(result.is_ok());

    // ok => priority normal
    let result = gas_oracle.gas_info(Priority::Normal).await;
    assert!(result.is_ok());

    // ok => priority high
    let result = gas_oracle.gas_info(Priority::High).await;
    assert!(result.is_ok());

    // ok => priority ASAP
    let result = gas_oracle.gas_info(Priority::ASAP).await;
    assert!(result.is_ok());

    // ok => invalid API key works (for some reason)
    let result = invalid_gas_oracle1.gas_info(Priority::Normal).await;
    assert!(result.is_ok());

    // ok => empty API key works (for some reason)
    let result = invalid_gas_oracle2.gas_info(Priority::Normal).await;
    assert!(result.is_ok());
}
