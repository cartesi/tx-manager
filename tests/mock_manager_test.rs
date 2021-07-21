mod common;

use common::mock_provider::*;
use tx_manager::{types::*, TransactionManager};

use offchain_utils::offchain_core::ethers;

use ethers::types::{Address, U256};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Tests
#[tokio::test]
async fn accounts_test() {
    let (_mockchain, manager) =
        new_manager(0, std::time::Duration::from_millis(100), 100.into());

    let accounts = manager.accounts().await.unwrap();
    assert_eq!(accounts.len(), 2);
    assert_eq!(accounts[0], Address::repeat_byte(1));
    assert_eq!(accounts[1], Address::repeat_byte(2));

    let account = manager.account(0).await.unwrap();
    assert_eq!(account, Address::repeat_byte(1));

    let account = manager.account(1).await.unwrap();
    assert_eq!(account, Address::repeat_byte(2));
}

#[tokio::test]
async fn simple_transaction_test() {
    let gas_price: U256 = 100.into();
    let (mockchain, manager) =
        new_manager(0, std::time::Duration::from_millis(100), gas_price);
    let account1 = manager.account(0).await.unwrap();
    let account2 = manager.account(1).await.unwrap();

    let transaction = Transaction {
        from: account1,
        to: account2,
        value: TransferValue::Nothing,
        call_data: None,
    };

    // Send transaction.
    assert!(!manager.label_exists(&0).await);
    assert!(manager
        .send_transaction(
            0,
            transaction,
            ResubmitStrategy {
                gas_multiplier: None,
                gas_price_multiplier: None,
                rate: 1,
            },
            10,
        )
        .await
        .unwrap());

    // Transaction Processing.
    assert!(if let Ok(TransactionState::Sending(SendState::Processing {
        ..
    })) = manager.transaction_state(&0).await
    {
        true
    } else {
        false
    });

    // Assert label exists.
    assert!(manager.label_exists(&0).await);
    tokio::task::yield_now().await;

    // Transaction Submitted.
    assert!(if let Ok(TransactionState::Sending(SendState::Submitted {
        ..
    })) = manager.transaction_state(&0).await
    {
        true
    } else {
        false
    });

    // Mine ten blocks and assert transaction confirming.
    for _ in 0..10 {
        mockchain.lock().await.mine_block(gas_price);
        tokio::task::yield_now().await;
        assert!(if let Ok(TransactionState::Sending(SendState::Confirming {
            ..
        })) = manager.transaction_state(&0).await
        {
            true
        } else {
            false
        });
    }

    // Ten blocks mined, assert Transaction Confirmed.
    mockchain.lock().await.mine_block(gas_price);
    tokio::task::yield_now().await;
    assert!(if let Ok(TransactionState::Finalized(
        FinalizedState::Confirmed { .. },
    )) = manager.transaction_state(&0).await
    {
        true
    } else {
        false
    });
}

#[tokio::test]
async fn transaction_resubmit_test() {
    let gas_price: U256 = 100.into();
    let threshold: U256 = 150.into();
    let (mockchain, manager) =
        new_manager(0, std::time::Duration::from_millis(100), gas_price);
    let account1 = manager.account(0).await.unwrap();
    let account2 = manager.account(1).await.unwrap();

    let transaction = Transaction {
        from: account1,
        to: account2,
        value: TransferValue::Nothing,
        call_data: None,
    };

    // Send transaction.
    assert!(!manager.label_exists(&0).await);
    assert!(manager
        .send_transaction(
            0,
            transaction,
            ResubmitStrategy {
                gas_multiplier: None,
                gas_price_multiplier: None,
                rate: 2,
            },
            10,
        )
        .await
        .unwrap());

    // Transaction Processing.
    assert!(if let Ok(TransactionState::Sending(SendState::Processing {
        ..
    })) = manager.transaction_state(&0).await
    {
        true
    } else {
        false
    });

    // Assert label exists.
    assert!(manager.label_exists(&0).await);
    tokio::task::yield_now().await;

    // Transaction Submitted.
    assert!(if let Ok(TransactionState::Sending(SendState::Submitted {
        ..
    })) = manager.transaction_state(&0).await
    {
        true
    } else {
        false
    });

    // Mine ten blocks and assert transaction Submitted (price under threshold).
    for _ in 0..10 {
        mockchain.lock().await.mine_block(threshold);
        tokio::task::yield_now().await;
        assert!(if let Ok(TransactionState::Sending(SendState::Submitted {
            ..
        })) = manager.transaction_state(&0).await
        {
            true
        } else {
            false
        });
    }

    mockchain.lock().await.set_price(120.into());

    // Mine ten blocks and assert transaction Submitted (price under threshold).
    for _ in 0..10 {
        mockchain.lock().await.mine_block(threshold);
        tokio::task::yield_now().await;
        assert!(if let Ok(TransactionState::Sending(SendState::Submitted {
            ..
        })) = manager.transaction_state(&0).await
        {
            true
        } else {
            false
        });
    }

    mockchain.lock().await.set_price(150.into());

    // Loop until transaction confirmed.
    let mut i = 0;
    loop {
        mockchain.lock().await.mine_block(threshold);
        tokio::task::yield_now().await;
        if let Ok(TransactionState::Finalized(FinalizedState::Confirmed {
            ..
        })) = manager.transaction_state(&0).await
        {
            break;
        } else {
            i += 1;
        };

        if i > 12 {
            assert!(false)
        }
    }
}

#[tokio::test]
async fn transaction_unclog_test() {
    let gas_price: U256 = 100.into();
    let threshold: U256 = 150.into();
    let (mockchain, manager) =
        new_manager(0, std::time::Duration::from_millis(100), gas_price);
    let account1 = manager.account(0).await.unwrap();
    let account2 = manager.account(1).await.unwrap();

    let transaction1 = Transaction {
        from: account1,
        to: account2,
        value: TransferValue::Nothing,
        call_data: None,
    };

    let transaction2 = Transaction {
        from: account1,
        to: account2,
        value: TransferValue::Nothing,
        call_data: None,
    };

    // Send transaction.
    assert!(!manager.label_exists(&0).await);
    assert!(manager
        .send_transaction(
            0,
            transaction1,
            ResubmitStrategy {
                gas_multiplier: None,
                gas_price_multiplier: None,
                rate: 2,
            },
            10,
        )
        .await
        .unwrap());

    // Transaction Processing.
    assert!(if let Ok(TransactionState::Sending(SendState::Processing {
        ..
    })) = manager.transaction_state(&0).await
    {
        true
    } else {
        false
    });

    // Assert label exists.
    assert!(manager.label_exists(&0).await);
    tokio::task::yield_now().await;

    // Transaction Submitted.
    assert!(if let Ok(TransactionState::Sending(SendState::Submitted {
        ..
    })) = manager.transaction_state(&0).await
    {
        true
    } else {
        false
    });

    // Mine ten blocks and assert transaction Submitted (price under threshold).
    for _ in 0..10 {
        mockchain.lock().await.mine_block(threshold);
        tokio::task::yield_now().await;
        assert!(if let Ok(TransactionState::Sending(SendState::Submitted {
            ..
        })) = manager.transaction_state(&0).await
        {
            true
        } else {
            false
        });
    }

    // Send another transaction.
    assert!(!manager.label_exists(&1).await);
    assert!(manager
        .send_transaction(
            1,
            transaction2,
            ResubmitStrategy {
                gas_multiplier: None,
                gas_price_multiplier: Some(1.5),
                rate: 2,
            },
            10,
        )
        .await
        .unwrap());

    // Transaction Processing.
    assert!(if let Ok(TransactionState::Sending(SendState::Processing {
        ..
    })) = manager.transaction_state(&1).await
    {
        true
    } else {
        false
    });

    // Assert label exists.
    assert!(manager.label_exists(&1).await);
    tokio::task::yield_now().await;

    // Second transaction Submitted.
    assert!(if let Ok(TransactionState::Sending(SendState::Submitted {
        ..
    })) = manager.transaction_state(&1).await
    {
        true
    } else {
        false
    });

    // Loop until first transaction confirmed.
    let mut i = 0;
    loop {
        mockchain.lock().await.mine_block(threshold);
        tokio::task::yield_now().await;
        if let Ok(TransactionState::Finalized(FinalizedState::Confirmed {
            ..
        })) = manager.transaction_state(&0).await
        {
            break;
        } else {
            i += 1;
        };

        if i > 12 {
            assert!(false)
        }
    }

    // Assert second transaction confirmed.
    assert!(if let Ok(TransactionState::Finalized(
        FinalizedState::Confirmed { .. },
    )) = manager.transaction_state(&1).await
    {
        true
    } else {
        false
    });
}

#[tokio::test]
async fn transaction_invalidate_test() {
    let gas_price: U256 = 100.into();
    let threshold: U256 = 150.into();
    let (mockchain, manager) =
        new_manager(0, std::time::Duration::from_millis(100), gas_price);
    let account1 = manager.account(0).await.unwrap();
    let account2 = manager.account(1).await.unwrap();

    let transaction = Transaction {
        from: account1,
        to: account2,
        value: TransferValue::Nothing,
        call_data: None,
    };

    // Send transaction.
    assert!(!manager.label_exists(&0).await);
    assert!(manager
        .send_transaction(
            0,
            transaction,
            ResubmitStrategy {
                gas_multiplier: None,
                gas_price_multiplier: None,
                rate: 2,
            },
            10,
        )
        .await
        .unwrap());

    // Transaction Processing.
    assert!(if let Ok(TransactionState::Sending(SendState::Processing {
        ..
    })) = manager.transaction_state(&0).await
    {
        true
    } else {
        false
    });

    // Assert label exists.
    assert!(manager.label_exists(&0).await);
    tokio::task::yield_now().await;

    // Transaction Submitted.
    assert!(if let Ok(TransactionState::Sending(SendState::Submitted {
        ..
    })) = manager.transaction_state(&0).await
    {
        true
    } else {
        false
    });

    // Mine ten blocks and assert transaction Submitted (price under threshold).
    for _ in 0..10 {
        mockchain.lock().await.mine_block(threshold);
        tokio::task::yield_now().await;
        assert!(if let Ok(TransactionState::Sending(SendState::Submitted {
            ..
        })) = manager.transaction_state(&0).await
        {
            true
        } else {
            false
        });
    }

    manager.invalidate_transaction(&0).await.unwrap();

    // InvalidateRequested.
    assert!(if let Ok(TransactionState::Invalidating(
        InvalidateState::InvalidateRequested { .. },
    )) = manager.transaction_state(&0).await
    {
        true
    } else {
        false
    });

    // Mine ten blocks and assert transaction InvalidateRequested (price under threshold).
    for _ in 0..10 {
        mockchain.lock().await.mine_block(threshold);
        tokio::task::yield_now().await;
        assert!(if let Ok(TransactionState::Invalidating(
            InvalidateState::InvalidateRequested { .. },
        )) = manager.transaction_state(&0).await
        {
            true
        } else {
            false
        });
    }

    mockchain.lock().await.set_price(150.into());

    // Loop until transaction confirmed.
    let mut i = 0;
    loop {
        mockchain.lock().await.mine_block(threshold);
        tokio::task::yield_now().await;
        if let Ok(TransactionState::Finalized(FinalizedState::Invalidated {
            ..
        })) = manager.transaction_state(&0).await
        {
            break;
        } else {
            i += 1;
        };

        if i > 12 {
            assert!(false)
        }
    }
}

///
/// Helpers
///

fn new_manager(
    max_retries: usize,
    max_delay: std::time::Duration,
    gas_price: U256,
) -> (
    Arc<Mutex<Mockchain>>,
    TransactionManager<MockProviderFactory, MockBlockSubscriber, usize>,
) {
    let mockchain = Mockchain::new(gas_price);

    let block_subscriber = MockBlockSubscriber::new(Arc::clone(&mockchain));
    let factory = MockProviderFactory::new(Arc::clone(&mockchain));

    let manager = TransactionManager::new(
        factory,
        block_subscriber,
        max_retries,
        max_delay,
    );

    (mockchain, manager)
}
