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

pub fn setup_tracing() {
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
}

pub struct Account {
    pub address: &'static str,
    pub private_key: &'static str,
}

pub const ACCOUNT1: Account = Account {
    address: "0x63fac9201494f0bd17b9892b9fae4d52fe3bd377",
    private_key: "8da4ef21b864d2cc526dbdb2a120bd2874c36c9d0a1fb7f8c63d7f7a8b41de8f",
};

pub const ACCOUNT2: Account = Account {
    address: "0xf30e6e20be8474393f2f2bbd61a52143d851c19b",
    private_key: "fda4ef21b864d2cc526dbdb2a120bd2874c36c9d0a1fb7f8c63d7f7a8b41de88",
};
