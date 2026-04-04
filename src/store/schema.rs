// SQLite schema initialization for persistent work items.
// Creates tables and indexes required by the store layer.

use anyhow::Result;
use rusqlite::Connection;

pub fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS items (
            id TEXT PRIMARY KEY,
            repo_path TEXT NOT NULL,
            title TEXT NOT NULL,
            description TEXT DEFAULT '',
            status TEXT DEFAULT 'backlog',
            priority TEXT DEFAULT 'medium',
            position REAL DEFAULT 0.0,
            parent_id TEXT,
            assignee TEXT,
            created_by TEXT NOT NULL,
            requires_approval INTEGER DEFAULT 0,
            auto_dispatch INTEGER DEFAULT 0,
            estimate TEXT,
            aid_agent TEXT,
            aid_verify TEXT,
            due_date TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            started_at TEXT,
            completed_at TEXT
        );
        CREATE TABLE IF NOT EXISTS item_labels (
            item_id TEXT NOT NULL,
            label TEXT NOT NULL,
            PRIMARY KEY (item_id, label),
            FOREIGN KEY (item_id) REFERENCES items(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS item_dependencies (
            item_id TEXT NOT NULL,
            depends_on TEXT NOT NULL,
            PRIMARY KEY (item_id, depends_on),
            FOREIGN KEY (item_id) REFERENCES items(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS item_aid_tasks (
            item_id TEXT NOT NULL,
            aid_task_id TEXT NOT NULL,
            status TEXT,
            created_at TEXT,
            PRIMARY KEY (item_id, aid_task_id),
            FOREIGN KEY (item_id) REFERENCES items(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            item_id TEXT NOT NULL,
            actor TEXT NOT NULL,
            action TEXT NOT NULL,
            detail TEXT,
            metadata TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (item_id) REFERENCES items(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS boards (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            filter TEXT,
            sort_by TEXT DEFAULT 'position',
            columns TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_items_status ON items(status);
        CREATE INDEX IF NOT EXISTS idx_items_priority ON items(priority);
        CREATE INDEX IF NOT EXISTS idx_items_parent_id ON items(parent_id);
        CREATE INDEX IF NOT EXISTS idx_items_status_position ON items(status, position);
        CREATE INDEX IF NOT EXISTS idx_events_item_id ON events(item_id);
        CREATE INDEX IF NOT EXISTS idx_events_created_at ON events(created_at);
        CREATE INDEX IF NOT EXISTS idx_item_labels_label ON item_labels(label);
        ",
    )?;
    Ok(())
}
