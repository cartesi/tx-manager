use async_trait::async_trait;
use std::fmt::Debug;
use std::io::ErrorKind;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::manager::State;

#[async_trait]
pub trait Database: Debug {
    type Error: std::error::Error;

    async fn set_state(&mut self, state: &State) -> Result<(), Self::Error>;

    async fn get_state(&self) -> Result<Option<State>, Self::Error>;

    async fn clear_state(&mut self) -> Result<(), Self::Error>;
}

// Implementation using the file system.

#[derive(Debug, thiserror::Error)]
pub enum FileSystemDatabaseError {
    #[error("could not create file: {0}")]
    CreateFile(std::io::Error),

    #[error("could not convert string to JSON: {0}")]
    ToJSON(serde_json::Error),

    #[error("could not write to file: {0}")]
    WriteToFile(std::io::Error),

    #[error("could not read file to string: {0}")]
    ReadFile(std::io::Error),

    #[error("could not parse JSON to string: {0}")]
    ParseJSON(serde_json::Error),

    #[error("could not delete file: {0}")]
    DeleteFile(std::io::Error),
}

#[derive(Debug)]
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
    type Error = FileSystemDatabaseError;

    async fn set_state(&mut self, state: &State) -> Result<(), Self::Error> {
        let mut file = fs::File::create(self.path)
            .await
            .map_err(Self::Error::CreateFile)?;
        let s =
            serde_json::to_string_pretty(state).map_err(Self::Error::ToJSON)?;
        file.write_all(s.as_bytes())
            .await
            .map_err(Self::Error::WriteToFile)?;
        file.sync_all().await.map_err(Self::Error::WriteToFile)?;
        return Ok(());
    }

    async fn get_state(&self) -> Result<Option<State>, Self::Error> {
        let file = fs::File::open(self.path).await;
        return match file {
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
            Err(err) => Err(Self::Error::ReadFile(err)),
            Ok(mut file) => {
                let mut s = String::new();
                file.read_to_string(&mut s)
                    .await
                    .map_err(Self::Error::ReadFile)?;
                let state = serde_json::de::from_str(&s)
                    .map_err(Self::Error::ParseJSON)?;
                return Ok(Some(state));
            }
        };
    }

    async fn clear_state(&mut self) -> Result<(), Self::Error> {
        Ok(fs::remove_file(self.path)
            .await
            .map_err(Self::Error::DeleteFile)?)
    }
}

// Unit tests for the file system database.

#[cfg(test)]
mod test {
    use ethers::types::{H160, H256};
    use serde_json::error::Category;
    use std::fs::{remove_file, File};
    use std::io::Write;
    use std::path::Path;

    use crate::database::{
        Database, FileSystemDatabase, FileSystemDatabaseError,
    };
    use crate::manager::State;
    use crate::transaction::{Priority, Transaction, Value};

    #[tokio::test]
    async fn test_file_system_database_set_state() {
        // setup
        let path_str = "./set_database.json";
        let path = Path::new(path_str);
        let mut database = FileSystemDatabase::new(path_str);
        let _ = remove_file(path);

        let state = State {
            nonce: Some(1u64.into()),
            transaction: Transaction {
                priority: Priority::Normal,
                from: H160::from_low_u64_ne(1u64),
                to: H160::from_low_u64_ne(2u64),
                value: Value::Number(5000u64.into()),
                confirmations: 0,
            },
            pending_transactions: vec![],
        };

        // ok => set state over empty state
        {
            assert!(!path.is_file());
            let result = database.set_state(&state).await;
            assert!(result.is_ok());
            assert!(path.is_file());
        }

        // ok => set state over preexisting state
        {
            let state = State {
                nonce: Some(2u64.into()),
                transaction: Transaction {
                    priority: Priority::High,
                    from: H160::from_low_u64_ne(5u64),
                    to: H160::from_low_u64_ne(6u64),
                    value: Value::Number(3000u64.into()),
                    confirmations: 5,
                },
                pending_transactions: vec![
                    H256::from_low_u64_ne(1400u64),
                    H256::from_low_u64_ne(1500u64),
                ],
            };
            assert!(path.is_file());
            let result = database.set_state(&state).await;
            assert!(result.is_ok());
            assert!(path.is_file());
        }

        // error => could not create the file (invalid path)
        {
            let path_str = "/bin/set_database.json";
            let path = Path::new(path_str);
            let mut database = FileSystemDatabase::new(path_str);

            assert!(!path.is_file());
            let result = database.set_state(&state).await;
            assert!(result.is_err());
            match result.err().unwrap() {
                FileSystemDatabaseError::CreateFile(err) => {
                    assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied)
                }
                _ => assert!(false, "expected CreateFile error"),
            };
            assert!(!path.is_file());
        }

        // Currently not testing the ToJSON and WriteToFile errors.

        // teardown
        assert!(database.clear_state().await.is_ok());
    }

    #[tokio::test]
    async fn test_file_system_database_get_state() {
        // setup
        let path_str = "./get_database.json";
        let path = Path::new(path_str);
        let mut database = FileSystemDatabase::new(path_str);
        let _ = remove_file(path);

        // ok => get empty state
        {
            assert!(!path.is_file());
            let result = database.get_state().await;
            assert!(result.is_ok());
            assert!(result.unwrap().is_none());
            assert!(!path.is_file());
        }

        // ok => get existing state
        {
            let original_state = State {
                nonce: Some(2u64.into()),
                transaction: Transaction {
                    priority: Priority::High,
                    from: H160::from_low_u64_ne(5u64),
                    to: H160::from_low_u64_ne(6u64),
                    value: Value::Number(3000u64.into()),
                    confirmations: 5,
                },
                pending_transactions: vec![
                    H256::from_low_u64_ne(1400u64),
                    H256::from_low_u64_ne(1500u64),
                ],
            };
            let result = database.set_state(&original_state).await;
            assert!(result.is_ok());
            let result = database.get_state().await;
            assert!(result.is_ok());
            let some_state = result.unwrap();
            assert!(some_state.is_some());
            let retrieved_state = some_state.unwrap();
            assert_eq!(original_state, retrieved_state);
        }

        // Currently not testing the ReadFile error.

        // error => could not parse the read file to JSON
        {
            let path_str = "./parse_json_test.json";
            let path = Path::new(path_str);
            let _ = remove_file(path);
            assert!(!path.is_file());
            let mut file = File::create(path).unwrap();
            file.write_all("this is not a JSON!".as_bytes()).unwrap();

            let database = FileSystemDatabase::new(path_str);
            let result = database.get_state().await;
            assert!(result.is_err());
            match result.err().unwrap() {
                FileSystemDatabaseError::ParseJSON(err) => {
                    assert_eq!(err.classify(), Category::Syntax)
                }
                _ => assert!(false, "expected ParseJSON error"),
            };
            assert!(path.is_file());
            remove_file(path).unwrap();
            assert!(!path.is_file());
        }

        // teardown
        assert!(database.clear_state().await.is_ok());
    }

    #[tokio::test]
    async fn test_file_system_database_clear_state() {
        // setup
        let path_str = "./clear_database.json";
        let path = Path::new(path_str);
        let _ = remove_file(path);
        assert!(File::create(path_str).is_ok());

        // ok => clearing the state
        assert!(path.is_file());
        let result = FileSystemDatabase::new(path_str).clear_state().await;
        assert!(result.is_ok());
        assert!(!path.is_file());

        // error => cannot clear an empty state
        assert!(!path.is_file());
        let result = FileSystemDatabase::new(path_str).clear_state().await;
        assert!(result.is_err());
        match result.err().unwrap() {
            FileSystemDatabaseError::DeleteFile(err) => {
                assert_eq!(err.kind(), std::io::ErrorKind::NotFound)
            }
            _ => assert!(false, "expected DeleteFile error"),
        };
        assert!(!path.is_file());
    }
}
