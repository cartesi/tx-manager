use anyhow::{bail, Result};
use async_trait::async_trait;

use tx_manager::manager::State;

#[derive(Debug, thiserror::Error)]
pub enum DatabaseOutput {
    #[error("database mock output: set state ok")]
    SetStateOk,

    #[error("database mock output: get state ok -- {0:?}")]
    GetStateOk(Option<State>),

    #[error("database mock output: clear state ok")]
    ClearStateOk,

    #[error("database mock output: set state error")]
    SetStateError,

    #[error("database mock output: get state error")]
    GetStateError,

    #[error("database mock output: clear state error")]
    ClearStateError,

    #[error("database mock output: unreachable error")]
    Unreachable,
}

pub struct Database {
    pub output: DatabaseOutput,
}

#[async_trait]
impl tx_manager::database::Database for Database {
    async fn set_state(&self, _: &State) -> Result<()> {
        match self.output {
            DatabaseOutput::SetStateOk => Ok(()),
            DatabaseOutput::SetStateError => {
                bail!(DatabaseOutput::SetStateError)
            }
            _ => bail!(DatabaseOutput::Unreachable),
        }
    }

    async fn get_state(&self) -> Result<Option<State>> {
        match &self.output {
            DatabaseOutput::GetStateOk(state) => Ok(state.clone()),
            DatabaseOutput::GetStateError => {
                bail!(DatabaseOutput::GetStateError)
            }
            _ => bail!(DatabaseOutput::Unreachable),
        }
    }

    async fn clear_state(&self) -> Result<()> {
        match self.output {
            DatabaseOutput::ClearStateOk => Ok(()),
            DatabaseOutput::ClearStateError => {
                bail!(DatabaseOutput::ClearStateError)
            }
            _ => bail!(DatabaseOutput::Unreachable),
        }
    }
}
