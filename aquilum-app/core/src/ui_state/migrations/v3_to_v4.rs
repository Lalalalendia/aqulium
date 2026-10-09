use super::super::error::UiStateError;
use rusqlite::Connection;

pub fn migrate_v3_to_v4(connection: &Connection) -> Result<(), UiStateError> {
    connection.execute_batch(
        "BEGIN IMMEDIATE;
         ALTER TABLE view_states
             ADD COLUMN fallback_scroll_anchor INTEGER NOT NULL DEFAULT 0;
         PRAGMA user_version = 4;
         COMMIT;",
    )?;
    Ok(())
}
