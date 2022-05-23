use anyhow::{anyhow, Result};
use async_trait::async_trait;

use tx_manager::manager::State;

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("database mock error: set state")]
    SetState,

    #[error("database mock error: get state")]
    GetState,

    #[error("database mock error: clear state")]
    ClearState,
}

#[derive(Debug)]
pub struct Database {
    pub set_state_output: Option<()>,
    pub get_state_output: Option<Option<State>>,
    pub clear_state_output: Option<()>,
}

impl Database {
    pub fn new() -> Self {
        Self {
            set_state_output: None,
            get_state_output: None,
            clear_state_output: None,
        }
    }
}

#[async_trait]
impl tx_manager::database::Database for Database {
    async fn set_state(&self, _: &State) -> Result<()> {
        self.set_state_output
            .ok_or(anyhow!(DatabaseError::SetState))
    }

    async fn get_state(&self) -> Result<Option<State>> {
        self.get_state_output
            .as_ref()
            .ok_or(anyhow!(DatabaseError::GetState))
            .map(|x| x.clone())
    }

    async fn clear_state(&self) -> Result<()> {
        self.clear_state_output
            .ok_or(anyhow!(DatabaseError::ClearState))
    }
}
