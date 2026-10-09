use super::database::UiStateDatabase;
use super::error::UiStateError;
use rusqlite::params;
use uuid::Uuid;

impl UiStateDatabase {
    pub fn mark_document_missing(
        &mut self,
        document_id: Uuid,
        now_ms: i64,
    ) -> Result<(), UiStateError> {
        self.connection.execute(
            "UPDATE documents SET status = 'missing', missing_since_ms = COALESCE(missing_since_ms, ?2)
             WHERE id = ?1",
            params![document_id.to_string(), now_ms],
        )?;
        Ok(())
    }

    pub fn purge_missing(&mut self, cutoff_ms: i64, limit: i64) -> Result<usize, UiStateError> {
        if limit <= 0 {
            return Ok(0);
        }
        let bounded_limit = limit.clamp(1, 5_000);
        let deleted = self.connection.execute(
            "DELETE FROM documents WHERE id IN (
                 SELECT id FROM documents
                 WHERE status = 'missing' AND missing_since_ms <= ?1
                 ORDER BY missing_since_ms LIMIT ?2
             )",
            params![cutoff_ms, bounded_limit],
        )?;
        Ok(deleted)
    }

    pub fn incremental_vacuum(&mut self, pages: i64) -> Result<(), UiStateError> {
        let bounded_pages = pages.clamp(0, 10_000);
        self.connection
            .execute_batch(&format!("PRAGMA incremental_vacuum({bounded_pages});"))?;
        Ok(())
    }
}
