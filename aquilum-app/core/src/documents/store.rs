use super::resolve::{SyncPoint, SyncPointLookup};
use rusqlite::{params, Connection, OptionalExtension};
use std::fmt;
use std::path::Path;

const SCHEMA_VERSION: i64 = 1;

#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    UnsupportedSchema(i64),
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(formatter, "{error}"),
            Self::UnsupportedSchema(version) => {
                write!(formatter, "documents store schema {version} is newer than this build")
            }
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

pub type StoreResult<T> = Result<T, StoreError>;

pub struct DocumentStore {
    connection: Connection,
}

impl DocumentStore {
    pub fn open(path: &Path) -> StoreResult<Self> {
        Self::prepare(Connection::open(path)?)
    }

    #[cfg(test)]
    pub fn memory() -> StoreResult<Self> {
        Self::prepare(Connection::open_in_memory()?)
    }

    fn prepare(connection: Connection) -> StoreResult<Self> {
        connection.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 2000;",
        )?;
        let version = connection.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?;
        match version {
            0 => connection.execute_batch(&format!(
                "CREATE TABLE replicas (key TEXT PRIMARY KEY, state BLOB NOT NULL);
                 CREATE TABLE replica_updates (
                     seq INTEGER PRIMARY KEY AUTOINCREMENT,
                     key TEXT NOT NULL,
                     data BLOB NOT NULL
                 );
                 CREATE INDEX replica_updates_key ON replica_updates (key, seq);
                 CREATE TABLE sync_points (
                     key TEXT PRIMARY KEY,
                     file_hash TEXT NOT NULL,
                     text_hash TEXT NOT NULL
                 );
                 PRAGMA user_version = {SCHEMA_VERSION};"
            ))?,
            SCHEMA_VERSION => {}
            newer => return Err(StoreError::UnsupportedSchema(newer)),
        }
        Ok(Self { connection })
    }

    pub fn load(&self, key: &str) -> StoreResult<Vec<Vec<u8>>> {
        let mut blobs: Vec<Vec<u8>> = self
            .connection
            .query_row("SELECT state FROM replicas WHERE key = ?1", [key], |row| row.get(0))
            .optional()?
            .into_iter()
            .collect();
        let mut statement =
            self.connection.prepare_cached("SELECT data FROM replica_updates WHERE key = ?1 ORDER BY seq")?;
        for update in statement.query_map([key], |row| row.get::<_, Vec<u8>>(0))? {
            blobs.push(update?);
        }
        Ok(blobs)
    }

    pub fn has_replica(&self, key: &str) -> StoreResult<bool> {
        let found = self.connection.query_row(
            "SELECT EXISTS (SELECT 1 FROM replicas WHERE key = ?1)
                 OR EXISTS (SELECT 1 FROM replica_updates WHERE key = ?1)",
            [key],
            |row| row.get::<_, bool>(0),
        )?;
        Ok(found)
    }

    pub fn append(&self, key: &str, update: &[u8]) -> StoreResult<()> {
        self.connection
            .prepare_cached("INSERT INTO replica_updates (key, data) VALUES (?1, ?2)")?
            .execute(params![key, update])?;
        Ok(())
    }

    pub fn compact(&mut self, key: &str, state: &[u8]) -> StoreResult<()> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO replicas (key, state) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET state = excluded.state",
            params![key, state],
        )?;
        transaction.execute("DELETE FROM replica_updates WHERE key = ?1", [key])?;
        transaction.commit()?;
        Ok(())
    }

    pub fn forget(&mut self, key: &str) -> StoreResult<()> {
        let transaction = self.connection.transaction()?;
        forget_in(&transaction, key)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn relocate(&mut self, from: &str, to: &str) -> StoreResult<()> {
        if from == to {
            return Ok(());
        }
        let transaction = self.connection.transaction()?;
        forget_in(&transaction, to)?;
        for table in ["replicas", "replica_updates", "sync_points"] {
            transaction.execute(&format!("UPDATE {table} SET key = ?2 WHERE key = ?1"), params![from, to])?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn sync_point(&self, key: &str) -> SyncPointLookup {
        let found = self
            .connection
            .query_row(
                "SELECT file_hash, text_hash FROM sync_points WHERE key = ?1",
                [key],
                |row| Ok(SyncPoint { file_hash: row.get(0)?, text_hash: row.get(1)? }),
            )
            .optional();
        match found {
            Ok(Some(point)) => SyncPointLookup::Found(point),
            Ok(None) => SyncPointLookup::Absent,
            Err(_) => SyncPointLookup::Unavailable,
        }
    }

    pub fn remember_sync_point(&self, key: &str, point: &SyncPoint) -> StoreResult<()> {
        self.connection.execute(
            "INSERT INTO sync_points (key, file_hash, text_hash) VALUES (?1, ?2, ?3)
             ON CONFLICT (key) DO UPDATE SET file_hash = excluded.file_hash, text_hash = excluded.text_hash",
            params![key, point.file_hash, point.text_hash],
        )?;
        Ok(())
    }
}

fn forget_in(connection: &Connection, key: &str) -> rusqlite::Result<()> {
    for table in ["replicas", "replica_updates", "sync_points"] {
        connection.execute(&format!("DELETE FROM {table} WHERE key = ?1"), [key])?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
