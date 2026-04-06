// SQLite store entry points and connection wrapper.
// Exports schema, item, and event helpers built on rusqlite.

pub mod artifacts;
pub mod events;
pub mod items;
pub mod schema;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use anyhow::{Result, anyhow};
use rusqlite::Connection;

#[allow(unused_imports)]
pub use artifacts::{
    GateResult, check_gate, delete_artifact, gen_artifact_id, get_artifact, insert_artifact,
    list_artifacts,
};
#[allow(unused_imports)]
pub use events::{insert_event, list_all_events, list_events};
#[allow(unused_imports)]
pub use items::{
    ItemFilter, ItemUpdate, delete_item, gen_id, get_item, insert_item, list_items, next_item,
    reorder_items, update_item, update_item_status,
};
pub use schema::init_schema;

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        configure_connection(&conn)?;
        init_schema(&conn)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn open_default() -> Result<Self> {
        let path = default_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        Self::open(&path)
    }

    pub fn connection(&self) -> MutexGuard<'_, Connection> {
        lock_connection(&self.conn)
    }

    pub fn into_inner(self) -> Connection {
        match self.conn.into_inner() {
            Ok(conn) => conn,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    pub fn with_connection<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let conn = self.connection();
        f(&conn)
    }
}

fn default_path() -> Result<PathBuf> {
    Ok(home_dir()?.join(".ai-board").join("db.sqlite3"))
}

fn configure_connection(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(())
}

fn lock_connection(conn: &Mutex<Connection>) -> MutexGuard<'_, Connection> {
    match conn.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn home_dir() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("HOME is not set"))
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;

    use uuid::Uuid;

    use super::Store;

    #[test]
    fn open_configures_connection_and_with_connection_runs_queries() {
        let path = test_db_path();
        let store = Store::open(&path).expect("open store");

        let journal_mode: String = store
            .with_connection(|conn| {
                Ok(conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))?)
            })
            .expect("read journal mode");
        let foreign_keys: i64 = store
            .connection()
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("read foreign keys pragma");

        assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
        assert_eq!(foreign_keys, 1);

        drop(store);
        cleanup_db_files(&path);
    }

    #[test]
    fn into_inner_returns_usable_connection() {
        let path = test_db_path();
        let store = Store::open(&path).expect("open store");

        let conn = store.into_inner();
        let value: i64 = conn
            .query_row("SELECT 1", [], |row| row.get(0))
            .expect("query inner connection");

        assert_eq!(value, 1);

        drop(conn);
        cleanup_db_files(&path);
    }

    fn test_db_path() -> std::path::PathBuf {
        temp_dir().join(format!("ai-board-store-{}.sqlite3", Uuid::new_v4()))
    }

    fn cleanup_db_files(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("sqlite3-shm"));
        let _ = std::fs::remove_file(path.with_extension("sqlite3-wal"));
    }
}
