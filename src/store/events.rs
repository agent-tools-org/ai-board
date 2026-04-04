// Event log persistence helpers for item activity history.
// Stores audit entries and lists recent events from SQLite.

use anyhow::Result;
use chrono::{DateTime, Local};
use rusqlite::{
    Connection, Row,
    params, params_from_iter,
    types::{Type, Value},
};

use crate::types::Event;

pub fn insert_event(
    conn: &Connection,
    item_id: &str,
    actor: &str,
    action: &str,
    detail: Option<&str>,
    metadata: Option<&serde_json::Value>,
) -> Result<()> {
    let metadata = metadata.map(serde_json::to_string).transpose()?;
    conn.execute(
        "INSERT INTO events (item_id, actor, action, detail, metadata, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
        params![item_id, actor, action, detail, metadata, now_text()],
    )?;
    Ok(())
}

pub fn list_events(conn: &Connection, item_id: &str, limit: Option<usize>) -> Result<Vec<Event>> {
    let mut sql = String::from(
        "SELECT id, item_id, actor, action, detail, metadata, created_at
         FROM events WHERE item_id = ? ORDER BY created_at DESC, id DESC",
    );
    let mut params = vec![Value::from(item_id.to_owned())];
    if let Some(limit) = limit {
        sql.push_str(" LIMIT ?");
        params.push(Value::from(limit as i64));
    }
    query_events(conn, &sql, params)
}

pub fn list_all_events(conn: &Connection, limit: Option<usize>) -> Result<Vec<Event>> {
    let mut sql = String::from(
        "SELECT id, item_id, actor, action, detail, metadata, created_at
         FROM events ORDER BY created_at DESC, id DESC",
    );
    let mut params = Vec::new();
    if let Some(limit) = limit {
        sql.push_str(" LIMIT ?");
        params.push(Value::from(limit as i64));
    }
    query_events(conn, &sql, params)
}

fn query_events(conn: &Connection, sql: &str, params: Vec<Value>) -> Result<Vec<Event>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params_from_iter(params.iter()), read_event_row)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

fn read_event_row(row: &Row<'_>) -> rusqlite::Result<Event> {
    let metadata = row
        .get::<_, Option<String>>(5)?
        .map(|value| serde_json::from_str(&value))
        .transpose()
        .map_err(json_error)?;
    Ok(Event {
        id: row.get(0)?,
        item_id: row.get(1)?,
        actor: row.get(2)?,
        action: row.get(3)?,
        detail: row.get(4)?,
        metadata,
        created_at: parse_time(&row.get::<_, String>(6)?).map_err(time_error)?,
    })
}

fn parse_time(value: &str) -> std::result::Result<DateTime<Local>, chrono::ParseError> {
    Ok(DateTime::parse_from_rfc3339(value)?.with_timezone(&Local))
}

fn now_text() -> String {
    Local::now().to_rfc3339()
}

fn json_error(error: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(5, Type::Text, Box::new(error))
}

fn time_error(error: chrono::ParseError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(6, Type::Text, Box::new(error))
}
