use async_trait::async_trait;
use std::io::ErrorKind;
use tokio::fs;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::manager::State;

#[async_trait]
pub trait Database {
    type Error;

    async fn set_state(&self, state: &State) -> Result<(), Self::Error>;

    async fn get_state(&self) -> Result<Option<State>, Self::Error>;

    async fn clear_state(&self) -> Result<(), Self::Error>;
}

pub struct FileSystemDatabase {
    path: String,
}

pub enum FileSystemDatabaseError {
    CreateFile(std::io::Error),
    OpenFile(std::io::Error),
    RemoveFile(std::io::Error),
    WriteToFile(std::io::Error),
    ReadFromFile(std::io::Error),
    Sync(std::io::Error),
    SerializeState(serde_json::error::Error),
    DeserializeState(serde_json::error::Error),
}

#[async_trait]
impl Database for FileSystemDatabase {
    type Error = FileSystemDatabaseError;

    async fn set_state(&self, state: &State) -> Result<(), Self::Error> {
        let mut file = fs::File::create(self.path.clone())
            .await
            .map_err(FileSystemDatabaseError::CreateFile)?;
        let s = serde_json::to_string(state)
            .map_err(FileSystemDatabaseError::SerializeState)?;
        file.write_all(s.as_bytes())
            .await
            .map_err(FileSystemDatabaseError::WriteToFile)?;
        // TODO
        file.sync_data()
            .await
            .map_err(FileSystemDatabaseError::Sync)?;
        return Ok(());
    }

    async fn get_state(&self) -> Result<Option<State>, Self::Error> {
        let file = fs::File::open(self.path.clone()).await;
        return match file {
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
            Err(err) => Err(FileSystemDatabaseError::OpenFile(err)),
            Ok(mut file) => {
                let mut s = String::new();
                file.read_to_string(&mut s)
                    .await
                    .map_err(FileSystemDatabaseError::ReadFromFile)?;
                let state = serde_json::de::from_str(&s)
                    .map_err(FileSystemDatabaseError::DeserializeState)?;
                return Ok(Some(state));
            }
        };
    }

    async fn clear_state(&self) -> Result<(), Self::Error> {
        return fs::remove_file(self.path.clone())
            .await
            .map_err(FileSystemDatabaseError::RemoveFile);
    }
}
