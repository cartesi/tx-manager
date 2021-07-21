#![allow(dead_code)]
pub mod mock_provider;
pub mod test_contract;

use test_contract::TestContract;

use offchain_utils::offchain_core::ethers;

use ethers::contract::ContractFactory;
use ethers::core::utils::{Geth, GethInstance, Solc};
use ethers::providers::{Http, Middleware, Provider};

use std::convert::TryFrom;
use std::sync::Arc;

pub async fn new_geth() -> (GethInstance, Arc<Provider<Http>>) {
    let geth = Geth::new().block_time(1u64).spawn();
    let provider = Provider::<Http>::try_from(geth.endpoint()).unwrap();
    let deployer = provider.get_accounts().await.unwrap()[0];
    (geth, Arc::new(provider.with_sender(deployer)))
}

pub async fn deploy_test_contract<M: Middleware>(
    client: Arc<M>,
    // ) -> Contract<M> {
) -> TestContract<M> {
    let contract_name = "TestContract";
    let path = "./tests/common/contract/TestContract.sol";
    let contracts = Solc::new(&path).build().unwrap();
    let contract = contracts.get(contract_name).unwrap();
    let abi = contract.abi.clone();
    let bytecode = contract.bytecode.clone();

    let factory = ContractFactory::new(abi, bytecode, Arc::clone(&client));
    let contract = factory.deploy(()).unwrap().send().await.unwrap();

    TestContract::new(contract.address(), client)
}
