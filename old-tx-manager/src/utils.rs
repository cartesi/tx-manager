use crate::error::{self, *};

use offchain_utils::middleware_factory::MiddlewareFactory;
use offchain_utils::offchain_core::ethers;
use offchain_utils::offchain_core::types::Block;

use ethers::providers::Middleware;
use ethers::types::U256;

use tokio::sync::broadcast;

pub fn should_retry<MF: MiddlewareFactory>(
    err: &error::ProviderError<MF::Middleware>,
) -> bool {
    match err {
        error::ProviderError::EthersErr { source, .. } => {
            <MF as MiddlewareFactory>::should_retry(source)
        }

        error::ProviderError::ReceiptConversionErr { .. } => true,

        error::ProviderError::TimeoutErr { source: _, err: _ } => true,

        _ => false,
    }
}

pub fn multiply(x: U256, y: Option<f64>) -> U256 {
    match y {
        None => x,
        Some(m) => {
            // Convert to u128
            let c1 = x.as_u128() as f64;

            // Multiply by multiplier
            let r = c1 * m;

            // Convert to u128
            let c2 = r.ceil() as u128;

            // Convert to U256
            let c3 = U256::from(c2);

            c3
        }
    }
}

pub async fn get_last_block<M: Middleware + 'static>(
    subscription: &mut broadcast::Receiver<Block>,
) -> WorkerResult<Block, M> {
    let mut block = None;
    loop {
        match subscription.try_recv() {
            Ok(b) => block = Some(b),

            // End of the queue
            Err(broadcast::error::TryRecvError::Empty) => {
                // We've either dequeued something, so we return it.
                if let Some(b) = block {
                    return Ok(b);
                }

                // Otherwise, wait on next block, handling errors as needed.
                match subscription.recv().await {
                    Err(broadcast::error::RecvError::Closed) => {
                        return SubscriberDroppedErr {}.fail();
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        continue;
                    }
                    Ok(block) => return Ok(block),
                }
            }

            Err(broadcast::error::TryRecvError::Closed) => {
                return SubscriberDroppedErr {}.fail()
            }

            Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
        }
    }
}
