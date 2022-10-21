use ethers::{
    middleware::signer::SignerMiddleware,
    prelude::k256::ecdsa::SigningKey,
    providers::Middleware,
    providers::{Http, Provider},
    signers::{LocalWallet, Signer, Wallet},
    types::Address,
};

use tx_manager::Chain;

use crate::{utilities, Account};

pub struct Net {
    pub provider: SignerMiddleware<Provider<Http>, Wallet<SigningKey>>,
    pub chain: Chain,
}

impl Net {
    pub fn new(provider_http_url: String, chain: Chain, account: &Account) -> Net {
        let provider = Provider::<Http>::try_from(provider_http_url.clone()).unwrap();
        let wallet: LocalWallet = account.clone().into();
        let provider = SignerMiddleware::new(provider, wallet.with_chain_id(chain.id));
        Net { provider, chain }
    }

    pub async fn get_balance_in_gwei(&self, account: &Account) -> u64 {
        let address: Address = account.clone().into();
        let balance_in_wei = self.provider.get_balance(address, None).await.unwrap();
        utilities::wei_to_gwei(balance_in_wei)
    }
}
