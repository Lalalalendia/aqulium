use super::super::error::UiStateError;
use rusqlite::Connection;

pub fn migrate_v5_to_v6(connection: &Connection) -> Result<(), UiStateError> {
    connection.execute_batch(
        "BEGIN IMMEDIATE;
         ALTER TABLE sessions ADD COLUMN graph_center_x REAL;
         ALTER TABLE sessions ADD COLUMN graph_center_y REAL;
         ALTER TABLE sessions ADD COLUMN graph_scale REAL;
         PRAGMA user_version = 6;
         COMMIT;",
    )?;
    Ok(())
}
