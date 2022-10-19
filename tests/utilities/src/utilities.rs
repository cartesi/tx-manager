use ethers::{
    core::rand::thread_rng,
    signers::{LocalWallet, Signer},
    types::{H160, U256},
};
use serde::{Deserialize, Serialize};
use tracing_subscriber::filter::EnvFilter;

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
        assert_eq!(
            "0x".to_string() + &hex::encode(wallet.address()),
            account.address
        );
        wallet
    }
}
