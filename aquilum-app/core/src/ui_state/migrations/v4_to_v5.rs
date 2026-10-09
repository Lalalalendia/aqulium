use super::super::error::UiStateError;
use rusqlite::Connection;

pub fn migrate_v4_to_v5(connection: &Connection) -> Result<(), UiStateError> {
    connection.execute_batch(
        "BEGIN IMMEDIATE;
         CREATE TABLE reader_states (
             workspace_id TEXT NOT NULL,
             book_file TEXT NOT NULL,
             current INTEGER NOT NULL,
             cfi TEXT,
             updated_at_ms INTEGER NOT NULL,
             PRIMARY KEY(workspace_id, book_file),
             FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
         );
         PRAGMA user_version = 5;
         COMMIT;",
    )?;
    Ok(())
}
