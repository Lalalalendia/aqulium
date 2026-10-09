use super::super::error::UiStateError;
use rusqlite::Connection;

pub fn migrate_v6_to_v7(connection: &Connection) -> Result<(), UiStateError> {
    connection.execute_batch(
        "BEGIN IMMEDIATE;
         ALTER TABLE workspaces ADD COLUMN home_page TEXT NOT NULL DEFAULT '';
         PRAGMA user_version = 7;
         COMMIT;",
    )?;
    Ok(())
}
