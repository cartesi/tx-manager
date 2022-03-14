mod common;

use offchain_utils::block_subscriber;
use offchain_utils::middleware_factory;
use offchain_utils::offchain_core::ethers;

use block_subscriber::{BlockSubscriber, BlockSubscriberHandle};
use tx_manager::{provider::Factory, TransactionManager};

use tx_manager::types::{FinalizedState, ResubmitStrategy, TransactionState};

use common::TestContract;
use common::*;

use ethers::{
    providers::{Http, Middleware, Provider, Ws},
    signers::{LocalWallet, Signer},
    types::{Address, U256},
    utils::GethInstance,
};

use std::sync::Arc;
use tokio::time::sleep;

type TestTxManager<PF> = TransactionManager<
    PF,
    BlockSubscriber<middleware_factory::WsProviderFactory>,
    usize,
>;

#[tokio::test]
async fn test_all() {
    let (geth, provider) = new_geth().await;
    let contract = deploy_test_contract(Arc::clone(&provider)).await;
    let account = provider.get_accounts().await.unwrap()[0];
    let (tx_manager, _subscriber_handle) = new_manager(&geth).await;

    let mut label_count: usize = 0;

    // Send normal transaction.
    let l = new_label(&mut label_count);
    increment_transaction(&tx_manager, &contract, l, account).await;
    wait(&tx_manager, l, 16, true).await;
    assert_increment(1, &contract).await;

    let (tx_manager_with_signer, _subscriber_handle, signer) =
        new_manager_with_signer(
            &geth,
            "380eb0f3d505f087e438eca80bc4df9a7faa24f868e69fc0440261a0fc0567dc",
        )
        .await;

    // Send transaction via non-exist signer.
    let l = new_label(&mut label_count);
    increment_transaction(&tx_manager_with_signer, &contract, l, signer).await;
    wait_err(&tx_manager_with_signer, l, 16).await;
}

///
///Helpers

fn new_label(label: &mut usize) -> usize {
    let ret = *label;
    *label += 1;
    ret
}

async fn wait<PF>(
    tx_manager: &TestTxManager<PF>,
    label: usize,
    iterations: usize,
    assert_final: bool,
) where
    PF: tx_manager::types::ProviderFactory + Send + Sync + 'static,
{
    let mut i = 0;
    loop {
        if i >= iterations {
            assert!(
                !assert_final,
                "assert not final error for label {}",
                label
            );
            return;
        }

        let state = tx_manager.transaction_state(&label).await.unwrap();
        if let TransactionState::Finalized(FinalizedState::Confirmed(_)) = state
        {
            assert!(assert_final, "assert final error for label {}", label);
            return;
        } else {
            i += 1;
            sleep(std::time::Duration::from_millis(500)).await;
        }
    }
}

async fn wait_err<PF>(
    tx_manager: &TestTxManager<PF>,
    label: usize,
    iterations: usize,
) where
    PF: tx_manager::types::ProviderFactory + Send + Sync + 'static,
{
    let mut i = 0;
    loop {
        if i >= iterations {
            panic!("transaction should fail with error");
        }

        let state_result = tx_manager.transaction_state(&label).await;
        if state_result.is_err() {
            return;
        } else {
            i += 1;
            sleep(std::time::Duration::from_millis(500)).await;
        }
    }
}

async fn increment_transaction<PF>(
    tx_manager: &TestTxManager<PF>,
    contract: &TestContract<Provider<Http>>,
    label: usize,
    from: Address,
) where
    PF: tx_manager::types::ProviderFactory + Send + Sync + 'static,
{
    let strategy = ResubmitStrategy {
        gas_multiplier: None,
        gas_price_multiplier: None,
        rate: 10,
    };

    let tx = contract.increment().from(from);
    assert!(tx_manager
        .send_transaction(label, tx, strategy, 4)
        .await
        .unwrap());
}

async fn new_manager(
    geth: &GethInstance,
) -> (
    TransactionManager<
        Factory<middleware_factory::HttpProviderFactory>,
        BlockSubscriber<middleware_factory::WsProviderFactory>,
        usize,
    >,
    BlockSubscriberHandle<Arc<Provider<Ws>>>,
) {
    let retry = 0;
    let delay = std::time::Duration::from_millis(100);
    let call_timeout = std::time::Duration::from_secs(1);
    let block_period = std::time::Duration::from_secs(3);

    let http_factory =
        middleware_factory::HttpProviderFactory::new(geth.endpoint()).unwrap();

    let ws_factory = middleware_factory::WsProviderFactory::new(
        geth.ws_endpoint(),
        retry,
        delay,
    )
    .await
    .unwrap();

    let (block_subscriber, subscriber_handle) =
        BlockSubscriber::create_and_start(
            ws_factory,
            block_period,
            retry,
            delay,
        );

    let factory = Factory::new(http_factory, call_timeout);

    let tx_manager: TransactionManager<_, _, usize> =
        TransactionManager::new(factory, block_subscriber, retry, delay);

    (tx_manager, subscriber_handle)
}

async fn new_manager_with_signer(
    geth: &GethInstance,
    key: &str,
) -> (
    TransactionManager<
        Factory<
            middleware_factory::LocalSignerFactory<
                middleware_factory::HttpProviderFactory,
            >,
        >,
        BlockSubscriber<middleware_factory::WsProviderFactory>,
        usize,
    >,
    BlockSubscriberHandle<Arc<Provider<Ws>>>,
    Address,
) {
    let retry = 0;
    let delay = std::time::Duration::from_millis(100);
    let call_timeout = std::time::Duration::from_secs(1);
    let block_period = std::time::Duration::from_secs(3);

    let http_factory =
        middleware_factory::HttpProviderFactory::new(geth.endpoint()).unwrap();

    let ws_factory = middleware_factory::WsProviderFactory::new(
        geth.ws_endpoint(),
        retry,
        delay,
    )
    .await
    .unwrap();

    let wallet: LocalWallet = key.parse().unwrap();
    let signer_factory = middleware_factory::LocalSignerFactory::new(
        http_factory,
        wallet.clone(),
    )
    .await
    .unwrap();

    let (block_subscriber, subscriber_handle) =
        BlockSubscriber::create_and_start(
            ws_factory,
            block_period,
            retry,
            delay,
        );

    let factory = Factory::new(signer_factory, call_timeout);

    let tx_manager: TransactionManager<_, _, usize> =
        TransactionManager::new(factory, block_subscriber, retry, delay);

    (tx_manager, subscriber_handle, wallet.address())
}

async fn assert_increment(
    value: usize,
    contract: &TestContract<Provider<Http>>,
) {
    let i = get_i(contract).await;
    assert_eq!(i, value.into());
}

async fn get_i(contract: &TestContract<Provider<Http>>) -> U256 {
    contract.i().call().await.unwrap()
}
