use ethers::{
    signers::{LocalWallet, Signer},
    types::{H160, U256},
};
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

pub struct Account {
    pub address: &'static str,
    pub private_key: &'static str,
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

pub const ACCOUNT1: Account = Account {
    address: "0x8bc95ac74684e81054f7cf0ac424aa6b2de3879b",
    private_key: "2e624d316fa3f655efd4a162c844f532d7dc8a487aecc571dcce255474fab9b0",
};

pub const ACCOUNT2: Account = Account {
    address: "0xcdccd122167e00be32e2a4f650094a4b745ce7c4",
    private_key: "7b0a26b368f1bb6dd3200c57fe26bee3de48860e4abc84abcb7de026a6681123",
};
