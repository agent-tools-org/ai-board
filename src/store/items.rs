// Work item CRUD, filtering, and scheduling queries for SQLite.
// Maps crate::types::WorkItem values to normalized store tables.

use anyhow::{Result, anyhow};
use chrono::{DateTime, Local};
use rand::Rng;
use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter, types::Value};

use super::artifacts::check_gate;
use crate::types::{Priority, Status, WorkItem};

const ITEM_SELECT: &str = "SELECT id, project, repo_path, title, description, status, priority, position, parent_id, assignee, created_by, requires_approval, auto_dispatch, estimate, aid_agent, aid_verify, due_date, created_at, updated_at, started_at, completed_at FROM items";
const STATUS_ORDER_SQL: &str =
    "CASE status WHEN 'backlog' THEN 0 WHEN 'ready' THEN 1 WHEN 'active' THEN 2 WHEN 'review' THEN 3 WHEN 'done' THEN 4 WHEN 'blocked' THEN 5 WHEN 'rejected' THEN 6 ELSE 7 END";
const PRIORITY_ORDER_SQL: &str =
    "CASE priority WHEN 'critical' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 WHEN 'low' THEN 3 ELSE 4 END";

#[derive(Default)]
pub struct ItemFilter {
    pub status: Option<Status>,
    pub priority: Option<Priority>,
    pub label: Option<String>,
    pub assignee: Option<String>,
    pub parent_id: Option<String>,
    pub project: Option<String>,
    pub repo_path: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Default)]
pub struct ItemUpdate {
    pub project: Option<String>,
    pub repo_path: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub priority: Option<Priority>,
    pub position: Option<f64>,
    pub labels: Option<Vec<String>>,
    pub parent_id: Option<Option<String>>,
    pub depends_on: Option<Vec<String>>,
    pub aid_task_ids: Option<Vec<String>>,
    pub aid_agent: Option<Option<String>>,
    pub aid_verify: Option<Option<String>>,
    pub estimate: Option<Option<String>>,
    pub assignee: Option<Option<String>>,
    pub requires_approval: Option<bool>,
    pub auto_dispatch: Option<bool>,
    pub due_date: Option<Option<String>>,
}

struct ItemRow {
    id: String,
    project: String,
    repo_path: String,
    title: String,
    description: String,
    status: String,
    priority: String,
    position: f64,
    parent_id: Option<String>,
    assignee: Option<String>,
    created_by: String,
    requires_approval: bool,
    auto_dispatch: bool,
    estimate: Option<String>,
    aid_agent: Option<String>,
    aid_verify: Option<String>,
    due_date: Option<String>,
    created_at: String,
    updated_at: String,
    started_at: Option<String>,
    completed_at: Option<String>,
}

pub fn insert_item(conn: &Connection, item: &WorkItem) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO items (id, project, repo_path, title, description, status, priority, position, parent_id, assignee, created_by, requires_approval, auto_dispatch, estimate, aid_agent, aid_verify, due_date, created_at, updated_at, started_at, completed_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            item.id, item.project, item.repo_path, item.title, item.description,
            enum_text(&item.status)?, enum_text(&item.priority)?, item.position, item.parent_id,
            item.assignee, item.created_by, item.requires_approval as i64,
            item.auto_dispatch as i64, item.estimate, item.aid_agent, item.aid_verify, item.due_date,
            item.created_at.to_rfc3339(), item.updated_at.to_rfc3339(),
            item.started_at.as_ref().map(DateTime::to_rfc3339),
            item.completed_at.as_ref().map(DateTime::to_rfc3339),
        ],
    )?;
    replace_labels(&tx, &item.id, &item.labels)?;
    replace_dependencies(&tx, &item.id, &item.depends_on)?;
    replace_aid_tasks(&tx, &item.id, &item.aid_task_ids)?;
    tx.commit()?;
    Ok(())
}

pub fn get_item(conn: &Connection, id: &str) -> Result<Option<WorkItem>> {
    let sql = format!("{ITEM_SELECT} WHERE id = ?");
    let row = conn.query_row(&sql, params![id], read_item_row).optional()?;
    row.map(|row| build_item(conn, row)).transpose()
}

pub fn list_items(conn: &Connection, filter: &ItemFilter) -> Result<Vec<WorkItem>> {
    let (sql, params) = build_list_query(filter);
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(params.iter()), read_item_row)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .map(|row| build_item(conn, row))
        .collect()
}

pub fn update_item_status(conn: &Connection, id: &str, status: Status, force: bool) -> Result<()> {
    let status = enum_text(&status)?;
    if !force {
        let gate = check_gate(conn, id, parse_status(&status)?)?;
        if !gate.allowed { return Err(anyhow!("Cannot move to {status}: missing {}", gate.missing.join(", "))); }
    }
    let now = now_text();
    conn.execute(
        "UPDATE items SET status = ?, updated_at = ?, started_at = CASE WHEN ? = 'active' AND started_at IS NULL THEN ? ELSE started_at END, completed_at = CASE WHEN ? = 'done' THEN ? ELSE NULL END WHERE id = ?",
        params![status, now, status, now, status, now, id],
    )?;
    Ok(())
}

pub fn update_item(conn: &Connection, id: &str, update: &ItemUpdate) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    if apply_item_update(&tx, id, update)? { tx.commit()?; }
    Ok(())
}

pub fn delete_item(conn: &Connection, id: &str) -> Result<()> { conn.execute("DELETE FROM items WHERE id = ?", params![id])?; Ok(()) }

pub fn next_item(conn: &Connection, project: &str, label: Option<&str>) -> Result<Option<WorkItem>> {
    let mut sql = format!("{ITEM_SELECT} WHERE project = ? AND status = 'ready' AND NOT EXISTS (SELECT 1 FROM item_dependencies d JOIN items dep ON dep.id = d.depends_on WHERE d.item_id = items.id AND dep.status != 'done')");
    let mut params = vec![Value::from(project.to_owned())];
    if let Some(label) = label { sql.push_str(" AND EXISTS (SELECT 1 FROM item_labels l WHERE l.item_id = items.id AND l.label = ?)"); params.push(Value::from(label.to_owned())); }
    sql.push_str(&format!(" ORDER BY {PRIORITY_ORDER_SQL}, position ASC LIMIT 1"));
    let row = conn.query_row(&sql, params_from_iter(params.iter()), read_item_row).optional()?;
    row.map(|row| build_item(conn, row)).transpose()
}

pub fn reorder_items(conn: &Connection, updates: &[(String, f64, String)]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for (id, position, status) in updates {
        let status = enum_text(&parse_status(status)?)?;
        let now = now_text();
        tx.execute(
            "UPDATE items SET position = ?, status = ?, updated_at = ?, started_at = CASE WHEN ? = 'active' AND started_at IS NULL THEN ? ELSE started_at END, completed_at = CASE WHEN ? = 'done' THEN ? ELSE NULL END WHERE id = ?",
            params![position, status, now, status, now, status, now, id],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn gen_id() -> String { format!("wi-{:04x}", rand::rng().random::<u16>()) }

fn build_list_query(filter: &ItemFilter) -> (String, Vec<Value>) {
    let mut sql = String::from(ITEM_SELECT);
    let mut clauses = Vec::new();
    let mut params = Vec::new();
    if let Some(status) = &filter.status { clauses.push("status = ?"); params.push(Value::from(enum_text(status).unwrap_or_default())); }
    if let Some(priority) = &filter.priority { clauses.push("priority = ?"); params.push(Value::from(enum_text(priority).unwrap_or_default())); }
    if let Some(label) = &filter.label { clauses.push("EXISTS (SELECT 1 FROM item_labels l WHERE l.item_id = items.id AND l.label = ?)"); params.push(Value::from(label.clone())); }
    if let Some(assignee) = &filter.assignee { clauses.push("assignee = ?"); params.push(Value::from(assignee.clone())); }
    if let Some(parent_id) = &filter.parent_id { clauses.push("parent_id = ?"); params.push(Value::from(parent_id.clone())); }
    if let Some(project) = &filter.project { clauses.push("project = ?"); params.push(Value::from(project.clone())); }
    if let Some(repo_path) = &filter.repo_path { clauses.push("repo_path = ?"); params.push(Value::from(repo_path.clone())); }
    if !clauses.is_empty() { sql.push_str(" WHERE "); sql.push_str(&clauses.join(" AND ")); }
    sql.push_str(&format!(" ORDER BY {STATUS_ORDER_SQL}, position ASC"));
    if let Some(limit) = filter.limit { sql.push_str(" LIMIT ?"); params.push(Value::from(limit as i64)); }
    (sql, params)
}

fn apply_item_update(conn: &Connection, id: &str, update: &ItemUpdate) -> Result<bool> {
    let mut sets = Vec::new();
    let mut params = Vec::new();
    if let Some(value) = &update.project { sets.push("project = ?"); params.push(Value::from(value.clone())); }
    if let Some(value) = &update.repo_path { sets.push("repo_path = ?"); params.push(Value::from(value.clone())); }
    if let Some(value) = &update.title { sets.push("title = ?"); params.push(Value::from(value.clone())); }
    if let Some(value) = &update.description { sets.push("description = ?"); params.push(Value::from(value.clone())); }
    if let Some(value) = &update.priority { sets.push("priority = ?"); params.push(Value::from(enum_text(value)?)); }
    if let Some(value) = update.position { sets.push("position = ?"); params.push(Value::from(value)); }
    if let Some(value) = &update.parent_id { sets.push("parent_id = ?"); params.push(optional_text(value.clone())); }
    if let Some(value) = &update.aid_agent { sets.push("aid_agent = ?"); params.push(optional_text(value.clone())); }
    if let Some(value) = &update.aid_verify { sets.push("aid_verify = ?"); params.push(optional_text(value.clone())); }
    if let Some(value) = &update.estimate { sets.push("estimate = ?"); params.push(optional_text(value.clone())); }
    if let Some(value) = &update.assignee { sets.push("assignee = ?"); params.push(optional_text(value.clone())); }
    if let Some(value) = update.requires_approval { sets.push("requires_approval = ?"); params.push(Value::from(value as i64)); }
    if let Some(value) = update.auto_dispatch { sets.push("auto_dispatch = ?"); params.push(Value::from(value as i64)); }
    if let Some(value) = &update.due_date { sets.push("due_date = ?"); params.push(optional_text(value.clone())); }
    let changed = !sets.is_empty() || update.labels.is_some() || update.depends_on.is_some() || update.aid_task_ids.is_some();
    if changed {
        sets.push("updated_at = ?");
        params.push(Value::from(now_text()));
        let sql = format!("UPDATE items SET {} WHERE id = ?", sets.join(", "));
        params.push(Value::from(id.to_owned()));
        conn.execute(&sql, params_from_iter(params.iter()))?;
    }
    if let Some(labels) = &update.labels { replace_labels(conn, id, labels)?; }
    if let Some(depends_on) = &update.depends_on { replace_dependencies(conn, id, depends_on)?; }
    if let Some(aid_task_ids) = &update.aid_task_ids { replace_aid_tasks(conn, id, aid_task_ids)?; }
    Ok(changed)
}

fn build_item(conn: &Connection, row: ItemRow) -> Result<WorkItem> {
    Ok(WorkItem {
        id: row.id.clone(),
        project: row.project,
        repo_path: row.repo_path,
        title: row.title,
        description: row.description,
        status: parse_status(&row.status)?,
        priority: parse_priority(&row.priority)?,
        position: row.position,
        labels: load_strings(conn, "SELECT label FROM item_labels WHERE item_id = ? ORDER BY label", &row.id)?,
        parent_id: row.parent_id,
        depends_on: load_strings(conn, "SELECT depends_on FROM item_dependencies WHERE item_id = ? ORDER BY depends_on", &row.id)?,
        aid_task_ids: load_strings(conn, "SELECT aid_task_id FROM item_aid_tasks WHERE item_id = ? ORDER BY created_at, aid_task_id", &row.id)?,
        aid_agent: row.aid_agent,
        aid_verify: row.aid_verify,
        estimate: row.estimate,
        assignee: row.assignee,
        created_by: row.created_by,
        requires_approval: row.requires_approval,
        auto_dispatch: row.auto_dispatch,
        created_at: parse_time(&row.created_at)?,
        updated_at: parse_time(&row.updated_at)?,
        started_at: parse_optional_time(row.started_at)?,
        completed_at: parse_optional_time(row.completed_at)?,
        due_date: row.due_date,
    })
}

fn read_item_row(row: &Row<'_>) -> rusqlite::Result<ItemRow> {
    Ok(ItemRow {
        id: row.get(0)?, project: row.get(1)?, repo_path: row.get(2)?, title: row.get(3)?,
        description: row.get(4)?, status: row.get(5)?, priority: row.get(6)?,
        position: row.get(7)?, parent_id: row.get(8)?, assignee: row.get(9)?,
        created_by: row.get(10)?, requires_approval: row.get::<_, i64>(11)? != 0,
        auto_dispatch: row.get::<_, i64>(12)? != 0, estimate: row.get(13)?,
        aid_agent: row.get(14)?, aid_verify: row.get(15)?, due_date: row.get(16)?,
        created_at: row.get(17)?, updated_at: row.get(18)?, started_at: row.get(19)?,
        completed_at: row.get(20)?,
    })
}

fn load_strings(conn: &Connection, sql: &str, id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![id], |row| row.get(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

fn replace_labels(conn: &Connection, item_id: &str, labels: &[String]) -> Result<()> {
    conn.execute("DELETE FROM item_labels WHERE item_id = ?", params![item_id])?;
    for label in labels { conn.execute("INSERT INTO item_labels (item_id, label) VALUES (?, ?)", params![item_id, label])?; }
    Ok(())
}

fn replace_dependencies(conn: &Connection, item_id: &str, depends_on: &[String]) -> Result<()> {
    conn.execute("DELETE FROM item_dependencies WHERE item_id = ?", params![item_id])?;
    for dependency in depends_on { conn.execute("INSERT INTO item_dependencies (item_id, depends_on) VALUES (?, ?)", params![item_id, dependency])?; }
    Ok(())
}

fn replace_aid_tasks(conn: &Connection, item_id: &str, aid_task_ids: &[String]) -> Result<()> {
    conn.execute("DELETE FROM item_aid_tasks WHERE item_id = ?", params![item_id])?;
    let now = now_text();
    for aid_task_id in aid_task_ids {
        conn.execute("INSERT INTO item_aid_tasks (item_id, aid_task_id, status, created_at) VALUES (?, ?, ?, ?)", params![item_id, aid_task_id, "linked", now])?;
    }
    Ok(())
}

fn enum_text<T: serde::Serialize>(value: &T) -> Result<String> {
    Ok(serde_json::to_string(value)?.trim_matches('"').to_owned())
}

fn parse_status(value: &str) -> Result<Status> {
    Ok(serde_json::from_str(&format!("\"{value}\""))?)
}

fn parse_priority(value: &str) -> Result<Priority> {
    Ok(serde_json::from_str(&format!("\"{value}\""))?)
}

fn parse_time(value: &str) -> Result<DateTime<Local>> {
    Ok(DateTime::parse_from_rfc3339(value)?.with_timezone(&Local))
}

fn parse_optional_time(value: Option<String>) -> Result<Option<DateTime<Local>>> { value.as_deref().map(parse_time).transpose() }

fn optional_text(value: Option<String>) -> Value { value.map(Value::from).unwrap_or(Value::Null) }
fn now_text() -> String { Local::now().to_rfc3339() }
