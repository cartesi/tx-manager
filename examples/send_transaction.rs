use ethers::{
    core::rand::thread_rng,
    middleware::signer::SignerMiddleware,
    providers::{Http, Provider},
    signers::{LocalWallet, Signer},
    types::{H160, U256},
};

use eth_tx_manager::{
    database::FileSystemDatabase,
    gas_oracle::DefaultGasOracle,
    manager::Configuration,
    transaction::{Priority, Transaction, Value},
    Chain, TransactionManager,
};

#[tokio::main]
async fn main() {
    let chain = Chain::new(1337);

    let provider = Provider::<Http>::try_from("http://localhost:8545").unwrap();
    let wallet = LocalWallet::new(&mut thread_rng()).with_chain_id(chain.id);
    let provider = SignerMiddleware::new(provider, wallet.clone());

    let gas_oracle = DefaultGasOracle::new();
    let database = FileSystemDatabase::new("database.json".to_string());
    let configuration = Configuration::default();

    let (manager, receipt) =
        TransactionManager::new(provider, gas_oracle, database, chain, configuration)
            .await
            .unwrap();
    assert!(receipt.is_none());

    let transaction = Transaction {
        from: wallet.address(),
        to: H160::random(),
        value: Value::Number(U256::from(1e9 as u64)),
        call_data: None,
    };

    let result = manager
        .send_transaction(transaction, 1, Priority::Normal)
        .await;
    assert!(result.is_err());
    println!("{:?}", result.err().unwrap());
}
