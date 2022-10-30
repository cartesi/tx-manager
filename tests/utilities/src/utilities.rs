use ethers::{
    core::rand::thread_rng,
    middleware::signer::SignerMiddleware,
    prelude::k256::ecdsa::SigningKey,
    providers::{Http, Middleware, Provider},
    signers::{LocalWallet, Signer, Wallet},
    types::{Address, H160, U256},
};
use serde::{Deserialize, Serialize};
use tracing_subscriber::filter::EnvFilter;

use tx_manager::Chain;

// ------------------------------------------------------------------------------------------------
// Macros
// ------------------------------------------------------------------------------------------------

#[macro_export]
macro_rules! assert_ok(
    ($result: expr) => {
        match $result {
            Ok(..) => {},
            Err(err) => panic!("expected Ok, got Err({:?})", err),
        }
    };
);

#[macro_export]
macro_rules! assert_err(
    ($result: expr, $expected: expr) => {
        match $result {
            Ok(..) => panic!("expected Err({:?}), got Ok(..)", $expected),
            Err(err) => assert_eq!(err.to_string(), $expected.to_string()),
        }
    };
);

// ------------------------------------------------------------------------------------------------
// ProviderWrapper
// ------------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ProviderWrapper {
    pub inner: SignerMiddleware<Provider<Http>, Wallet<SigningKey>>,
    pub chain: Chain,
}

pub struct Balance {
    account1: Account,
    account2: Account,
    account1_before: u64,
    account2_before: u64,
}

impl ProviderWrapper {
    pub fn new(provider_http_url: String, chain: Chain, account: &Account) -> ProviderWrapper {
        let provider = Provider::<Http>::try_from(provider_http_url.clone()).unwrap();
        let wallet: LocalWallet = account.clone().into();
        let provider = SignerMiddleware::new(provider, wallet.with_chain_id(chain.id));
        ProviderWrapper {
            inner: provider,
            chain,
        }
    }

    pub async fn get_balance_in_gwei(&self, account: &Account) -> u64 {
        let address: Address = account.clone().into();
        let balance_in_wei = self.inner.get_balance(address, None).await.unwrap();
        wei_to_gwei(balance_in_wei)
    }

    pub async fn get_balance(&self, account1: Account, account2: Account) -> Balance {
        let account1_before = self.get_balance_in_gwei(&account1).await;
        let account2_before = self.get_balance_in_gwei(&account2).await;

        println!(
            "[TEST LOG] Account 1 balance (before) = {:?}",
            account1_before
        );
        println!(
            "[TEST LOG] Account 2 balance (before) = {:?}",
            account2_before
        );
        Balance {
            account1,
            account2,
            account1_before,
            account2_before,
        }
    }

    pub async fn check_transaction_balance(&self, balance: Balance, amount: u64) {
        let account1_after = self.get_balance_in_gwei(&balance.account1).await;
        let account2_after = self.get_balance_in_gwei(&balance.account2).await;
        let delta1 = (account1_after as i64) - (balance.account1_before as i64);
        let delta2 = (account2_after as i64) - (balance.account2_before as i64);

        println!(
            "[TEST LOG] Account 1 balance (after) = {:?}",
            account1_after
        );
        println!(
            "[TEST LOG] Account 2 balance (after) = {:?}",
            account2_after
        );
        println!("[TEST LOG] Delta 1 = {:?}", delta1);
        println!("[TEST LOG] Delta 2 = {:?}", delta2);

        assert!(account1_after <= balance.account1_before - amount); // weak assertion
        assert!(account2_after == balance.account2_before + amount);
    }
}

// ------------------------------------------------------------------------------------------------
// Account
// ------------------------------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct Account {
    pub address: String,
    pub private_key: String,
}

impl Account {
    pub fn random() -> Self {
        let wallet = LocalWallet::new(&mut thread_rng());
        Account {
            address: hex::encode(wallet.address()),
            private_key: hex::encode(wallet.signer().to_bytes()),
        }
    }
}

impl From<Account> for H160 {
    fn from(account: Account) -> Self {
        account.address.parse().unwrap()
    }
}

impl From<Account> for LocalWallet {
    fn from(account: Account) -> Self {
        let wallet = account.private_key.parse::<LocalWallet>().unwrap();
        let address = account
            .address
            .strip_prefix("0x")
            .unwrap_or(&account.address);
        assert_eq!(hex::encode(wallet.address()), address);
        wallet
    }
}

// ------------------------------------------------------------------------------------------------
// Miscellaneous
// ------------------------------------------------------------------------------------------------

pub fn wei_to_gwei(wei: U256) -> u64 {
    wei.as_u64() / (1e9 as u64)
}

pub fn gwei_to_wei(gwei: u64) -> U256 {
    U256::from(gwei * (1e9 as u64))
}

pub fn setup_tracing() {
    // RUST_LOG="tx_manager::manager=trace"
    let format = tracing_subscriber::fmt::format()
        .without_time()
        .with_target(false)
        .with_level(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false)
        .with_source_location(false)
        .compact();
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .event_format(format)
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();
    // TODO: log to file
}
