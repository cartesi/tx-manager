#![allow(dead_code)]
pub mod mock_provider;

pub use testcontract_mod::TestContract;

use offchain_utils::offchain_core::ethers;

use ethers::contract::{abigen, ContractFactory};
use ethers::core::utils::{Geth, GethInstance};
use ethers::providers::{Http, Middleware, Provider};

use hex;
use std::convert::TryFrom;
use std::sync::Arc;

abigen!(TestContract, "./tests/common/contract/TestContract.abi",);

pub async fn new_geth() -> (GethInstance, Arc<Provider<Http>>) {
    let geth = Geth::new().block_time(1u64).spawn();
    let provider = Provider::<Http>::try_from(geth.endpoint()).unwrap();
    let deployer = provider.get_accounts().await.unwrap()[0];
    (geth, Arc::new(provider.with_sender(deployer)))
}

pub async fn deploy_test_contract<M: Middleware>(
    client: Arc<M>,
) -> TestContract<M> {
    let bytecode = hex::decode(include_bytes!("./contract/TestContract.bin"))
        .unwrap()
        .into();
    let abi = testcontract_mod::TESTCONTRACT_ABI.clone();

    let factory = ContractFactory::new(abi, bytecode, Arc::clone(&client));
    let contract = factory.deploy(()).unwrap().send().await.unwrap();

    TestContract::new(contract.address(), client)
}
