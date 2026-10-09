use super::super::error::UiStateError;
use rusqlite::Connection;

pub fn migrate_v2_to_v3(connection: &Connection) -> Result<(), UiStateError> {
    connection.execute_batch(
        "BEGIN IMMEDIATE;
         CREATE TABLE document_versions (
             id TEXT PRIMARY KEY,
             document_id TEXT NOT NULL,
             parent_version_id TEXT,
             kind TEXT NOT NULL,
             label TEXT,
             content_hash TEXT NOT NULL,
             state_vector BLOB NOT NULL,
             storage_key TEXT NOT NULL,
             created_at_ms INTEGER NOT NULL,
             FOREIGN KEY(document_id) REFERENCES documents(id) ON DELETE CASCADE,
             FOREIGN KEY(parent_version_id) REFERENCES document_versions(id) ON DELETE SET NULL
         );
         CREATE INDEX document_versions_timeline
             ON document_versions(document_id, created_at_ms DESC);
         PRAGMA user_version = 3;
         COMMIT;",
    )?;
    Ok(())
}
