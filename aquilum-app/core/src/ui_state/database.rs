use super::error::UiStateError;
use super::migrations::migrate;
use rusqlite::Connection;
use std::path::Path;

pub struct UiStateDatabase {
    pub connection: Connection,
}

impl UiStateDatabase {
    pub fn open(path: &Path) -> Result<Self, UiStateError> {
        let connection = Connection::open(path)?;
        migrate(&connection)?;
        Ok(Self { connection })
    }

    #[cfg(test)]
    pub fn memory() -> Result<Self, UiStateError> {
        let connection = Connection::open_in_memory()?;
        migrate(&connection)?;
        Ok(Self { connection })
    }
}
