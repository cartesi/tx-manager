use async_trait::async_trait;
use std::fmt::Debug;
use std::time::{Duration, Instant};
use tx_manager::time::Time;

#[derive(Debug)]
pub struct MockTime;

#[async_trait]
impl Time for MockTime {
    async fn sleep(&self, _: Duration) {}

    fn elapsed(&self, _: Instant) -> Duration {
        Duration::from_secs(1)
    }
}
