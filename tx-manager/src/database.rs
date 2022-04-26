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

pub fn new_file_system_database(path: &'static str) -> FileSystemDatabase {
    return FileSystemDatabase { path };
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
