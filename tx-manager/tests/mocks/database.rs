use anyhow::{bail, Result};
use async_trait::async_trait;

use tx_manager::manager::State;

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("database mock error: set state error")]
    SetStateError,

    #[error("database mock error: get state error")]
    GetStateError,

    #[error("database mock error: clear state error")]
    ClearStateError,
}

pub struct Database {
    pub set_state: bool,
    pub get_state: (bool, Option<State>),
    pub clear_state: bool,
}

impl Database {
    pub fn new() -> Self {
        Self {
            set_state: false,
            get_state: (false, None),
            clear_state: false,
        }
    }

    pub fn reset(&mut self) {
        self.set_state = false;
        self.get_state = (false, None);
        self.clear_state = false;
    }
}

#[async_trait]
impl tx_manager::database::Database for Database {
    async fn set_state(&self, _: &State) -> Result<()> {
        if self.set_state {
            Ok(())
        } else {
            bail!(DatabaseError::SetStateError)
        }
    }

    async fn get_state(&self) -> Result<Option<State>> {
        if self.get_state.0 {
            Ok(self.get_state.1.clone())
        } else {
            bail!(DatabaseError::GetStateError)
        }
    }

    async fn clear_state(&self) -> Result<()> {
        if self.clear_state {
            Ok(())
        } else {
            bail!(DatabaseError::ClearStateError)
        }
    }
}
