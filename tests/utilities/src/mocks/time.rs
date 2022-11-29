use async_trait::async_trait;
use eth_tx_manager::time::Time;
use std::{
    fmt::Debug,
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct MockTime;

#[async_trait]
impl Time for MockTime {
    async fn sleep(&self, _: Duration) {}

    fn elapsed(&self, _: Instant) -> Duration {
        Duration::from_secs(1)
    }
}
