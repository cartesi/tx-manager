use async_trait::async_trait;
use std::fmt::Debug;
use std::time::{Duration, Instant};

#[async_trait]
pub trait Time: Clone + Debug {
    async fn sleep(&self, duration: Duration);

    fn elapsed(&self, start: Instant) -> Duration;
}

#[derive(Clone, Debug)]
pub struct DefaultTime;

#[async_trait]
impl Time for DefaultTime {
    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }

    fn elapsed(&self, start: Instant) -> Duration {
        Duration::from_millis(start.elapsed().as_millis() as u64)
    }
}
