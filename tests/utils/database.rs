use async_trait::async_trait;

use tx_manager::transaction;

#[derive(Debug)]
pub struct Database {
    pub set_state_output: Option<()>,
    pub get_state_output: Option<Option<transaction::PersistentState>>,
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

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("database mock error: set state")]
    SetState,

    #[error("database mock error: get state")]
    GetState,

    #[error("database mock error: clear state")]
    ClearState,
}

#[async_trait]
impl tx_manager::database::Database for Database {
    type Error = DatabaseError;

    async fn set_state(
        &mut self,
        _: &transaction::PersistentState,
    ) -> Result<(), Self::Error> {
        unsafe { GLOBAL.set_state_n += 1 };
        self.set_state_output.ok_or(DatabaseError::SetState)
    }

    async fn get_state(
        &self,
    ) -> Result<Option<transaction::PersistentState>, Self::Error> {
        unsafe { GLOBAL.get_state_n += 1 };
        self.get_state_output
            .as_ref()
            .ok_or(DatabaseError::GetState)
            .map(|x| x.clone())
    }

    async fn clear_state(&mut self) -> Result<(), Self::Error> {
        unsafe { GLOBAL.clear_state_n += 1 };
        self.clear_state_output.ok_or(DatabaseError::ClearState)
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
