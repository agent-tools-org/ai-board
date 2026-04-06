// Artifact persistence and workflow gate checks for work items.
// Exports: CRUD helpers, gate evaluation, and artifact ID generation.

use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, Local};
use rand::Rng;
use rusqlite::{Connection, OptionalExtension, Row, params, types::Type};

use crate::types::{Artifact, ArtifactType, Status};

const DESIGN_GATE: &str = "design_doc or investigation (final status)";
const AUDIT_GATE: &str = "audit_report";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateResult {
    pub allowed: bool,
    pub missing: Vec<String>,
}

pub fn insert_artifact(conn: &Connection, artifact: &Artifact) -> Result<()> {
    validate_status(&artifact.status)?;
    if artifact.title.trim().is_empty() {
        bail!("artifact title cannot be empty");
    }
    conn.execute(
        "INSERT INTO artifacts (id, item_id, artifact_type, title, path, content, status, created_by, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            artifact.id,
            artifact.item_id,
            enum_text(&artifact.artifact_type)?,
            artifact.title,
            artifact.path,
            artifact.content,
            artifact.status,
            artifact.created_by,
            artifact.created_at.to_rfc3339(),
            artifact.updated_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

pub fn list_artifacts(conn: &Connection, item_id: &str) -> Result<Vec<Artifact>> {
    let mut stmt = conn.prepare(
        "SELECT id, item_id, artifact_type, title, path, content, status, created_by, created_at, updated_at
         FROM artifacts WHERE item_id = ? ORDER BY updated_at DESC, id DESC",
    )?;
    let rows = stmt.query_map(params![item_id], read_artifact_row)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

pub fn get_artifact(conn: &Connection, id: &str) -> Result<Option<Artifact>> {
    conn.query_row(
        "SELECT id, item_id, artifact_type, title, path, content, status, created_by, created_at, updated_at
         FROM artifacts WHERE id = ?",
        params![id],
        read_artifact_row,
    )
    .optional()
    .map_err(Into::into)
}

pub fn delete_artifact(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM artifacts WHERE id = ?", params![id])?;
    Ok(())
}

pub fn check_gate(conn: &Connection, item_id: &str, target_status: Status) -> Result<GateResult> {
    let current_status: String = conn
        .query_row("SELECT status FROM items WHERE id = ?", params![item_id], |row| row.get(0))
        .optional()?
        .ok_or_else(|| anyhow!("work item not found: {item_id}"))?;
    let target_status = enum_text(&target_status)?;
    if current_status == "ready" && target_status == "active" {
        return Ok(required_gate(has_final_design(conn, item_id)?, DESIGN_GATE));
    }
    if current_status == "active" && matches!(target_status.as_str(), "review" | "done") {
        return Ok(required_gate(has_audit_report(conn, item_id)?, AUDIT_GATE));
    }
    Ok(GateResult { allowed: true, missing: Vec::new() })
}

pub fn gen_artifact_id() -> String { format!("af-{:04x}", rand::rng().random::<u16>()) }

fn required_gate(ok: bool, missing: &str) -> GateResult {
    GateResult { allowed: ok, missing: (!ok).then(|| vec![missing.to_owned()]).unwrap_or_default() }
}

fn has_final_design(conn: &Connection, item_id: &str) -> Result<bool> {
    has_matching_artifact(
        conn,
        item_id,
        "artifact_type IN ('design_doc', 'investigation') AND status = 'final'",
    )
}

fn has_audit_report(conn: &Connection, item_id: &str) -> Result<bool> {
    has_matching_artifact(conn, item_id, "artifact_type = 'audit_report'")
}

fn has_matching_artifact(conn: &Connection, item_id: &str, clause: &str) -> Result<bool> {
    let sql = format!("SELECT EXISTS(SELECT 1 FROM artifacts WHERE item_id = ? AND {clause})");
    Ok(conn.query_row(&sql, params![item_id], |row| row.get::<_, i64>(0))? != 0)
}

fn read_artifact_row(row: &Row<'_>) -> rusqlite::Result<Artifact> {
    Ok(Artifact {
        id: row.get(0)?,
        item_id: row.get(1)?,
        artifact_type: parse_artifact_type(&row.get::<_, String>(2)?).map_err(type_error)?,
        title: row.get(3)?,
        path: row.get(4)?,
        content: row.get(5)?,
        status: row.get(6)?,
        created_by: row.get(7)?,
        created_at: parse_time(&row.get::<_, String>(8)?).map_err(time_error)?,
        updated_at: parse_time(&row.get::<_, String>(9)?).map_err(time_error)?,
    })
}

fn enum_text<T: serde::Serialize>(value: &T) -> Result<String> {
    Ok(serde_json::to_string(value)?.trim_matches('"').to_owned())
}

fn parse_artifact_type(value: &str) -> std::result::Result<ArtifactType, serde_json::Error> {
    serde_json::from_str(&format!("\"{value}\""))
}

fn parse_time(value: &str) -> std::result::Result<DateTime<Local>, chrono::ParseError> {
    Ok(DateTime::parse_from_rfc3339(value)?.with_timezone(&Local))
}

fn validate_status(status: &str) -> Result<()> {
    if matches!(status, "draft" | "final") {
        return Ok(());
    }
    bail!("invalid artifact status: {status}")
}

fn type_error(error: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(2, Type::Text, Box::new(error))
}

fn time_error(error: chrono::ParseError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(8, Type::Text, Box::new(error))
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use crate::store::schema::init_schema;

    use super::*;

    #[test]
    fn check_gate_requires_final_design_and_audit_artifacts() {
        let conn = Connection::open_in_memory().expect("open memory db");
        init_schema(&conn).expect("init schema");
        conn.execute(
            "INSERT INTO items (id, project, repo_path, title, created_by, created_at, updated_at, status, priority) VALUES ('wi-1', '', '', 'Test', 'human', '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00', 'ready', 'medium')",
            [],
        )
        .expect("insert item");
        assert_eq!(check_gate(&conn, "wi-1", Status::Active).expect("gate").missing, vec![DESIGN_GATE.to_owned()]);

        insert_artifact(&conn, &artifact("wi-1", ArtifactType::DesignDoc, "draft")).expect("insert draft design");
        assert!(!check_gate(&conn, "wi-1", Status::Active).expect("gate").allowed);

        insert_artifact(&conn, &artifact("wi-1", ArtifactType::Investigation, "final")).expect("insert final investigation");
        assert!(check_gate(&conn, "wi-1", Status::Active).expect("gate").allowed);

        conn.execute("UPDATE items SET status = 'active' WHERE id = 'wi-1'", []).expect("activate item");
        assert_eq!(check_gate(&conn, "wi-1", Status::Review).expect("gate").missing, vec![AUDIT_GATE.to_owned()]);

        insert_artifact(&conn, &artifact("wi-1", ArtifactType::AuditReport, "draft")).expect("insert audit");
        assert!(check_gate(&conn, "wi-1", Status::Done).expect("gate").allowed);
    }

    fn artifact(item_id: &str, artifact_type: ArtifactType, status: &str) -> Artifact {
        let now = Local::now();
        Artifact {
            id: gen_artifact_id(),
            item_id: item_id.to_owned(),
            artifact_type,
            title: "Doc".to_owned(),
            path: None,
            content: String::new(),
            status: status.to_owned(),
            created_by: "human:test".to_owned(),
            created_at: now,
            updated_at: now,
        }
    }
}
