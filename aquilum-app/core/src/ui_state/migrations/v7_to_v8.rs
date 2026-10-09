use super::super::error::UiStateError;
use rusqlite::Connection;

pub fn migrate_v7_to_v8(connection: &Connection) -> Result<(), UiStateError> {
    connection.execute_batch(
        "BEGIN IMMEDIATE;
         DROP TABLE IF EXISTS document_versions;
         PRAGMA user_version = 8;
         COMMIT;",
    )?;
    Ok(())
}
