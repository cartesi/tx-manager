use anyhow::{bail, Result};
use async_trait::async_trait;
use std::io::ErrorKind;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::manager::State;

#[async_trait]
pub trait Database {
    async fn set_state(&self, state: &State) -> Result<()>;

    async fn get_state(&self) -> Result<Option<State>>;

    async fn clear_state(&self) -> Result<()>;
}

// Implementation using the file system.

pub struct FileSystemDatabase {
    path: &'static str,
}

impl FileSystemDatabase {
    pub fn new(path: &'static str) -> FileSystemDatabase {
        return FileSystemDatabase { path };
    }
}

#[async_trait]
impl Database for FileSystemDatabase {
    async fn set_state(&self, state: &State) -> Result<()> {
        let mut file = fs::File::create(self.path).await?;
        let s = serde_json::to_string_pretty(state)?;
        file.write_all(s.as_bytes()).await?;
        file.sync_all().await?;
        return Ok(());
    }

    async fn get_state(&self) -> Result<Option<State>> {
        let file = fs::File::open(self.path).await;
        return match file {
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
            Err(err) => bail!(err),
            Ok(mut file) => {
                let mut s = String::new();
                file.read_to_string(&mut s).await?;
                let state = serde_json::de::from_str(&s)?;
                return Ok(Some(state));
            }
        };
    }

    async fn clear_state(&self) -> Result<()> {
        return Ok(fs::remove_file(self.path).await?);
    }
}

// Unit tests for the file system database.

#[cfg(test)]
mod test {
    use ethers::types::{H160, H256};
    use std::fs::File;
    use std::path::Path;

    use crate::database::{Database, FileSystemDatabase};
    use crate::manager::State;
    use crate::transaction::{Priority, Transaction, Value};

    #[tokio::test]
    async fn test_file_system_database_set_state() {
        // setup
        let path_str = "./set_database.json";
        let path = Path::new(path_str);
        let database = FileSystemDatabase::new(path_str);
        let _ = database.clear_state().await;

        // ok => set state over empty state
        let state1 = State {
            nonce: Some(1.into()),
            transaction: Transaction {
                priority: Priority::Normal,
                from: to_h160(1),
                to: to_h160(2),
                value: Value::Number(5000.into()),
                confirmations: 0,
            },
            pending_transactions: vec![],
        };
        assert!(!path.is_file());
        let result = database.set_state(&state1).await;
        assert!(result.is_ok());
        assert!(path.is_file());

        // ok => set state over preexisting state
        let state2 = State {
            nonce: Some(2.into()),
            transaction: Transaction {
                priority: Priority::High,
                from: to_h160(5),
                to: to_h160(6),
                value: Value::Number(3000.into()),
                confirmations: 5,
            },
            pending_transactions: vec![to_h256(1400), to_h256(1500)],
        };
        assert!(path.is_file());
        let result = database.set_state(&state2).await;
        assert!(result.is_ok());
        assert!(path.is_file());

        // teardown
        assert!(database.clear_state().await.is_ok());
    }

    #[tokio::test]
    async fn test_file_system_database_get_state() {
        // setup
        let path_str = "./get_database.json";
        let path = Path::new(path_str);
        let database = FileSystemDatabase::new(path_str);
        let _ = database.clear_state().await;

        // ok => get empty state
        assert!(!path.is_file());
        let result = database.get_state().await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
        assert!(!path.is_file());

        // ok => get existing state
        let original_state = State {
            nonce: Some(2.into()),
            transaction: Transaction {
                priority: Priority::High,
                from: to_h160(5),
                to: to_h160(6),
                value: Value::Number(3000.into()),
                confirmations: 5,
            },
            pending_transactions: vec![to_h256(1400), to_h256(1500)],
        };
        let result = database.set_state(&original_state).await;
        assert!(result.is_ok());
        let result = database.get_state().await;
        assert!(result.is_ok());
        let some_state = result.unwrap();
        assert!(some_state.is_some());
        let retrieved_state = some_state.unwrap();
        assert_eq!(original_state, retrieved_state);

        // teardown
        assert!(database.clear_state().await.is_ok());
    }

    #[tokio::test]
    async fn test_file_system_database_clear_state() {
        // setup
        let path_str = "./clear_database.json";
        let path = Path::new(path_str);
        assert!(File::create(path_str).is_ok());

        // ok => clearing the state
        assert!(path.is_file());
        let result = FileSystemDatabase::new(path_str).clear_state().await;
        assert!(result.is_ok());
        assert!(!path.is_file());

        // error => cannot clear an empty state
        assert!(!path.is_file());
        let result = FileSystemDatabase::new(path_str).clear_state().await;
        assert!(result.is_err(), "{:?}", result.err());
        assert!(!path.is_file());
    }

    // auxiliary functions

    fn to_h160(n: u64) -> H160 {
        return H160::from_low_u64_ne(n);
    }

    fn to_h256(n: u64) -> H256 {
        return H256::from_low_u64_ne(n);
    }
}
