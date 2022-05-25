use anyhow::{anyhow, Result};
use async_trait::async_trait;

use tx_manager::manager;

#[derive(Debug)]
pub struct Database {
    pub set_state_output: Option<()>,
    pub get_state_output: Option<Option<manager::State>>,
    pub clear_state_output: Option<()>,
}

impl Database {
    pub fn new() -> Self {
        Global::setup();
        Self {
            set_state_output: None,
            get_state_output: None,
            clear_state_output: None,
        }
    }

    pub fn global() -> &'static Global {
        unsafe { &GLOBAL }
    }
}

#[async_trait]
impl tx_manager::database::Database for Database {
    async fn set_state(&mut self, _: &manager::State) -> Result<()> {
        unsafe { GLOBAL.set_state_n += 1 };
        self.set_state_output
            .ok_or(anyhow!(DatabaseError::SetState))
    }

    async fn get_state(&self) -> Result<Option<manager::State>> {
        unsafe { GLOBAL.get_state_n += 1 };
        self.get_state_output
            .as_ref()
            .ok_or(anyhow!(DatabaseError::GetState))
            .map(|x| x.clone())
    }

    async fn clear_state(&mut self) -> Result<()> {
        unsafe { GLOBAL.clear_state_n += 1 };
        self.clear_state_output
            .ok_or(anyhow!(DatabaseError::ClearState))
    }
}

pub struct Global {
    pub set_state_n: i32,
    pub get_state_n: i32,
    pub clear_state_n: i32,
}

static mut GLOBAL: Global = Global::default();

impl Global {
    const fn default() -> Global {
        Global {
            set_state_n: 0,
            get_state_n: 0,
            clear_state_n: 0,
        }
    }

    fn setup() {
        unsafe {
            GLOBAL = Global::default();
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("database mock error: set state")]
    SetState,

    #[error("database mock error: get state")]
    GetState,

    #[error("database mock error: clear state")]
    ClearState,
}
