use async_trait::async_trait;
use std::fmt::Debug;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct Time;

#[async_trait]
impl tx_manager::time::Time for Time {
    async fn sleep(&self, _: Duration) {}

    fn elapsed(&self, _: Instant) -> Duration {
        Duration::from_secs(1)
    }
}
