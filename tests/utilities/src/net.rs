/*
use ethers::{
    middleware::signer::SignerMiddleware,
    prelude::{k256::ecdsa::SigningKey, Wallet},
    providers::{Http, Provider},
    signers::{LocalWallet, Signer},
    types::Chain,
};

pub struct Net {
    pub provider_http_url: String,
    pub chain: Chain,
}

impl Net {
    pub fn provider(
        &self,
        signer: &LocalWallet,
    ) -> SignerMiddleware<Provider<Http>, Wallet<SigningKey>> {
        let provider = Provider::<Http>::try_from(self.provider_http_url.clone()).unwrap();
        SignerMiddleware::new(provider, signer.clone())
    }

    pub fn create_wallet<T: Into<String>>(&self, private_key: T) -> LocalWallet {
        private_key
            .into()
            .parse::<LocalWallet>()
            .unwrap()
            .with_chain_id(self.chain)
    }
}
*/
