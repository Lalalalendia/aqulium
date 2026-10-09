use super::super::error::UiStateError;
use rusqlite::Connection;

pub fn migrate_v1_to_v2(connection: &Connection) -> Result<(), UiStateError> {
    connection.execute_batch(
        "PRAGMA foreign_keys = OFF;
         PRAGMA legacy_alter_table = ON;
         BEGIN IMMEDIATE;
         ALTER TABLE documents RENAME TO documents_v1;
         CREATE TABLE documents (
             id TEXT PRIMARY KEY,
             workspace_id TEXT NOT NULL,
             relative_path TEXT NOT NULL,
             status TEXT NOT NULL DEFAULT 'active',
             missing_since_ms INTEGER,
             UNIQUE(workspace_id, id),
             FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
         );
         INSERT INTO documents SELECT id, workspace_id, relative_path, status, missing_since_ms
             FROM documents_v1;
         DROP TABLE documents_v1;
         CREATE UNIQUE INDEX active_document_path
             ON documents(workspace_id, relative_path) WHERE status = 'active';
         PRAGMA user_version = 2;
         COMMIT;
         PRAGMA legacy_alter_table = OFF;
         PRAGMA foreign_keys = ON;",
    )?;
    let violations =
        connection.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get::<_, i64>(0)
        })?;
    if violations != 0 {
        return Err(UiStateError::Database {
            message: format!("foreign key violations after migration: {violations}"),
        });
    }
    Ok(())
}
