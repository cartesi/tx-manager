use async_trait::async_trait;
use std::fmt::Debug;
use std::io::ErrorKind;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::transaction::PersistentState;

#[async_trait]
pub trait Database: Debug {
    type Error: std::error::Error;

    async fn set_state(
        &mut self,
        state: &PersistentState,
    ) -> Result<(), Self::Error>;

    async fn get_state(&self) -> Result<Option<PersistentState>, Self::Error>;

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
    path: String,
}

impl FileSystemDatabase {
    pub fn new(path: String) -> FileSystemDatabase {
        FileSystemDatabase { path }
    }
}

#[async_trait]
impl Database for FileSystemDatabase {
    type Error = FileSystemDatabaseError;

    async fn set_state(
        &mut self,
        state: &PersistentState,
    ) -> Result<(), Self::Error> {
        let mut file = fs::File::create(self.path.clone())
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

    async fn get_state(&self) -> Result<Option<PersistentState>, Self::Error> {
        let file = fs::File::open(self.path.clone()).await;

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
        Ok(fs::remove_file(self.path.clone())
            .await
            .map_err(Self::Error::DeleteFile)?)
    }
}

// Unit tests for the file system database.

#[cfg(test)]
mod test {
    use ethers::types::{H160, H256};
    use serde_json::error::Category;
    use serial_test::serial;
    use std::fs::{remove_file, File};
    use std::io::Write;
    use std::path::PathBuf;

    use crate::database::{
        Database, FileSystemDatabase, FileSystemDatabaseError,
    };
    use crate::transaction::{PersistentState, StaticTxData, SubmittedTxs};
    use crate::transaction::{Priority, Transaction, Value};

    /// Auxiliary.
    fn setup(str: String) -> (PathBuf, FileSystemDatabase) {
        let path = PathBuf::from(&str);
        let database = FileSystemDatabase::new(str);
        let _ = remove_file(path.as_path());
        (path, database)
    }

    #[tokio::test]
    #[serial]
    async fn test_file_system_database_set_state_ok_empty_state() {
        let state = PersistentState {
            tx_data: StaticTxData {
                nonce: 1u64.into(),
                transaction: Transaction {
                    from: H160::from_low_u64_ne(1u64),
                    to: H160::from_low_u64_ne(2u64),
                    value: Value::Number(5000u64.into()),
                    call_data: None,
                },
                priority: Priority::Normal,
                confirmations: 0,
            },
            submitted_txs: SubmittedTxs::new(),
        };

        let (path, mut database) = setup("./set_database.json".to_string());
        let path = path.as_path();

        assert!(!path.is_file());
        let result = database.set_state(&state).await;
        assert!(result.is_ok());
        assert!(path.is_file());
        remove_file(path).unwrap();
        assert!(!path.is_file());
    }

    #[tokio::test]
    #[serial]
    async fn test_file_system_database_set_state_ok_existing_state() {
        let state = PersistentState {
            tx_data: StaticTxData {
                nonce: 2u64.into(),
                transaction: Transaction {
                    from: H160::from_low_u64_ne(5u64),
                    to: H160::from_low_u64_ne(6u64),
                    value: Value::Number(3000u64.into()),
                    call_data: None,
                },
                priority: Priority::High,
                confirmations: 5,
            },
            submitted_txs: SubmittedTxs {
                txs_hashes: vec![
                    H256::from_low_u64_ne(1400u64),
                    H256::from_low_u64_ne(1500u64),
                ],
            },
        };

        let (path, mut database) = setup("./set_database.json".to_string());
        let path = path.as_path();

        assert!(!path.is_file());
        let result = database.set_state(&state).await;
        assert!(result.is_ok());
        assert!(path.is_file());
        let result = database.set_state(&state).await;
        assert!(result.is_ok());
        assert!(path.is_file());
        remove_file(path).unwrap();
        assert!(!path.is_file());
    }

    #[tokio::test]
    #[serial]
    async fn test_file_system_database_set_state_error() {
        // error => could not create the file (invalid path)

        let state = PersistentState {
            tx_data: StaticTxData {
                nonce: 1u64.into(),
                transaction: Transaction {
                    from: H160::from_low_u64_ne(1u64),
                    to: H160::from_low_u64_ne(2u64),
                    value: Value::Number(5000u64.into()),
                    call_data: None,
                },
                priority: Priority::Normal,
                confirmations: 0,
            },
            submitted_txs: SubmittedTxs::new(),
        };

        let path_str = "/bin/set_database.json".to_string();
        let path = PathBuf::from(&path_str);
        let path = path.as_path();
        let mut database = FileSystemDatabase::new(path_str.clone());

        assert!(!path.is_file());
        let result = database.set_state(&state).await;
        assert!(result.is_err());
        let err = result.as_ref().err().unwrap();
        assert!(
            matches!(err, FileSystemDatabaseError::CreateFile(err) if err.kind() == std::io::ErrorKind::PermissionDenied),
            "expected CreateFile::PermissionDenied error, got {}",
            err
        );
        assert!(!path.is_file());
    }

    // Currently not testing the ToJSON and WriteToFile errors.

    #[tokio::test]
    #[serial]
    async fn test_file_system_database_get_state_ok_empty_state() {
        let (path, database) = setup("./get_database.json".to_string());
        assert!(!path.is_file());
        let result = database.get_state().await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
        assert!(!path.is_file());
    }

    #[tokio::test]
    #[serial]
    async fn test_file_system_database_get_state_ok_existing_state() {
        let original_state = PersistentState {
            tx_data: StaticTxData {
                nonce: 2u64.into(),
                transaction: Transaction {
                    from: H160::from_low_u64_ne(5u64),
                    to: H160::from_low_u64_ne(6u64),
                    value: Value::Number(3000u64.into()),
                    call_data: None,
                },
                priority: Priority::High,
                confirmations: 5,
            },
            submitted_txs: SubmittedTxs {
                txs_hashes: vec![
                    H256::from_low_u64_ne(1400u64),
                    H256::from_low_u64_ne(1500u64),
                ],
            },
        };

        let (path, mut database) = setup("./get_database.json".to_string());
        let path = path.as_path();

        assert!(!path.is_file());
        let result = database.set_state(&original_state).await;
        assert!(result.is_ok());
        assert!(path.is_file());
        let result = database.get_state().await;
        assert!(result.is_ok());
        let some_state = result.unwrap();
        assert!(some_state.is_some());
        let retrieved_state = some_state.unwrap();
        assert_eq!(original_state, retrieved_state);

        assert!(path.is_file());
        remove_file(path).unwrap();
        assert!(!path.is_file());
    }

    // Currently not testing the ReadFile error.

    #[tokio::test]
    #[serial]
    async fn test_file_system_database_get_state_error() {
        // error => could not parse the read file to JSON

        let path_str = "./parse_json_test.json".to_string();
        let path = PathBuf::from(path_str.clone());
        let path = path.as_path();
        let _ = remove_file(path);
        assert!(!path.is_file());
        let mut file = File::create(path).unwrap();
        file.write_all("this is not a JSON!".as_bytes()).unwrap();

        let database = FileSystemDatabase::new(path_str.clone());
        let result = database.get_state().await;
        assert!(result.is_err());
        let err = result.as_ref().err().unwrap();
        assert!(
            matches!(err, FileSystemDatabaseError::ParseJSON(err) if err.classify() == Category::Syntax),
            "expected ParseJSON::Syntax error, got {}",
            err
        );

        assert!(path.is_file());
        remove_file(path).unwrap();
        assert!(!path.is_file());
    }

    #[tokio::test]
    #[serial]
    async fn test_file_system_database_clear_state_ok() {
        let path_str = "./clear_database.json".to_string();
        let (path, mut database) = setup(path_str.clone());
        assert!(File::create(path_str.clone()).is_ok());

        let result = database.clear_state().await;
        assert!(result.is_ok());
        assert!(!path.is_file());
    }

    #[tokio::test]
    #[serial]
    async fn test_file_system_database_clear_state_error_empty_state() {
        let path_str = "./clear_database.json".to_string();
        let (path, mut database) = setup(path_str.clone());

        let result = database.clear_state().await;
        assert!(result.is_err());
        let err = result.as_ref().err().unwrap();
        assert!(
            matches!(err, FileSystemDatabaseError::DeleteFile(err) if err.kind() == std::io::ErrorKind::NotFound),
            "expected DeleteFile::NotFound error. got {}",
            err
        );

        assert!(!path.is_file());
    }
}
