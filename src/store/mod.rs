// SQLite store entry points and connection wrapper.
// Exports schema, item, and event helpers built on rusqlite.

pub mod events;
pub mod items;
pub mod schema;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use rusqlite::Connection;

#[allow(unused_imports)]
pub use events::{insert_event, list_all_events, list_events};
#[allow(unused_imports)]
pub use items::{
    ItemFilter, ItemUpdate, delete_item, gen_id, get_item, insert_item, list_items, next_item,
    reorder_items, update_item, update_item_status,
};
pub use schema::init_schema;

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        configure_connection(&conn)?;
        init_schema(&conn)?;
        Ok(Self { conn })
    }

    pub fn open_default() -> Result<Self> {
        let path = default_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        Self::open(&path)
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn into_inner(self) -> Connection {
        self.conn
    }
}

fn default_path() -> Result<PathBuf> {
    Ok(std::env::current_dir()?.join(".ai-board").join("db.sqlite3"))
}

fn configure_connection(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(())
}
