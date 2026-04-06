// Agent-facing endpoints for queue pickup, progress reporting, and review submission.
// Exports: routes() and handlers under /api/agent using shared store/types modules.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    api::{ApiError, AppState, agent_actor, default_project, missing_item},
    store::{ItemUpdate, get_item, insert_event, next_item, update_item, update_item_status},
    types::{Status, WorkItem},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/agent/next", get(next_ready))
        .route("/api/agent/items/{id}/claim", post(claim))
        .route("/api/agent/items/{id}/complete", post(complete))
        .route("/api/agent/items/{id}/block", post(block))
        .route("/api/agent/items/{id}/submit", post(submit))
        .route("/api/agent/items/{id}/note", post(note))
}

#[derive(Deserialize)]
struct NextQuery {
    project: Option<String>,
    label: Option<String>,
}

#[derive(Deserialize)]
struct ClaimBody {
    assignee: String,
}

#[derive(Deserialize)]
struct CompleteBody {
    summary: String,
    aid_task_id: Option<String>,
    files_changed: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct BlockBody {
    reason: String,
}

#[derive(Deserialize, Default)]
struct SubmitBody {
    summary: Option<String>,
}

#[derive(Deserialize)]
struct NoteBody {
    actor: String,
    note: String,
}

async fn next_ready(
    State(store): State<AppState>,
    Query(query): Query<NextQuery>,
) -> Result<Json<WorkItem>, ApiError> {
    let project = default_project(query.project)
        .ok_or_else(|| ApiError::bad_request("project query parameter is required"))?;
    store
        .with_connection(|conn| next_item(conn, &project, query.label.as_deref()))
        .map_err(ApiError::internal)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("no ready item found"))
}

async fn claim(
    State(store): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ClaimBody>,
) -> Result<Json<WorkItem>, ApiError> {
    let actor = agent_actor(&body.assignee)?;
    store
        .with_connection(|conn| {
            ensure_item(conn, &id)?;
            update_item(
                conn,
                &id,
                &ItemUpdate {
                    assignee: Some(Some(actor.clone())),
                    ..ItemUpdate::default()
                },
            )?;
            update_item_status(conn, &id, Status::Active)?;
            insert_event(conn, &id, &actor, "claimed", None, None)?;
            get_item(conn, &id)?.ok_or_else(|| anyhow::anyhow!("missing"))
        })
        .map(Json)
        .map_err(|error| map_missing(error, &id))
}

async fn complete(
    State(store): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<CompleteBody>,
) -> Result<Json<WorkItem>, ApiError> {
    if body.summary.trim().is_empty() {
        return Err(ApiError::bad_request("summary cannot be empty"));
    }
    store
        .with_connection(|conn| {
            let mut item = fetch_item(conn, &id)?;
            let actor = current_actor(&item);
            if let Some(task_id) = &body.aid_task_id {
                push_aid_task(&mut item, task_id);
                update_item(
                    conn,
                    &id,
                    &ItemUpdate {
                        aid_task_ids: Some(item.aid_task_ids.clone()),
                        ..ItemUpdate::default()
                    },
                )?;
            }
            let status = if item.requires_approval { Status::Review } else { Status::Done };
            update_item_status(conn, &id, status)?;
            let metadata = json!({
                "summary": body.summary,
                "aid_task_id": body.aid_task_id,
                "files_changed": body.files_changed.clone().unwrap_or_default(),
            });
            insert_event(conn, &id, &actor, "completed", Some(&body.summary), Some(&metadata))?;
            get_item(conn, &id)?.ok_or_else(|| anyhow::anyhow!("missing"))
        })
        .map(Json)
        .map_err(|error| map_missing(error, &id))
}

async fn block(
    State(store): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<BlockBody>,
) -> Result<Json<WorkItem>, ApiError> {
    if body.reason.trim().is_empty() {
        return Err(ApiError::bad_request("reason cannot be empty"));
    }
    agent_transition(store, id, Status::Blocked, "blocked", body.reason, None).await
}

async fn submit(
    State(store): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<SubmitBody>>,
) -> Result<Json<WorkItem>, ApiError> {
    let summary = body.and_then(|payload| payload.0.summary);
    agent_transition(store, id, Status::Review, "submitted", summary.unwrap_or_default(), None).await
}

async fn note(
    State(store): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<NoteBody>,
) -> Result<Json<WorkItem>, ApiError> {
    if body.note.trim().is_empty() {
        return Err(ApiError::bad_request("note cannot be empty"));
    }
    let actor = agent_actor(&body.actor)?;
    store
        .with_connection(|conn| {
            ensure_item(conn, &id)?;
            insert_event(conn, &id, &actor, "note", Some(&body.note), None)?;
            get_item(conn, &id)?.ok_or_else(|| anyhow::anyhow!("missing"))
        })
        .map(Json)
        .map_err(|error| map_missing(error, &id))
}

async fn agent_transition(
    store: AppState,
    id: String,
    status: Status,
    action: &str,
    detail: String,
    metadata: Option<serde_json::Value>,
) -> Result<Json<WorkItem>, ApiError> {
    store
        .with_connection(|conn| {
            let item = fetch_item(conn, &id)?;
            let actor = current_actor(&item);
            update_item_status(conn, &id, status)?;
            let maybe_detail = (!detail.trim().is_empty()).then_some(detail.as_str());
            insert_event(conn, &id, &actor, action, maybe_detail, metadata.as_ref())?;
            get_item(conn, &id)?.ok_or_else(|| anyhow::anyhow!("missing"))
        })
        .map(Json)
        .map_err(|error| map_missing(error, &id))
}

fn fetch_item(conn: &rusqlite::Connection, id: &str) -> anyhow::Result<WorkItem> {
    get_item(conn, id)?.ok_or_else(|| anyhow::anyhow!("missing"))
}

fn ensure_item(conn: &rusqlite::Connection, id: &str) -> anyhow::Result<()> {
    fetch_item(conn, id).map(|_| ())
}

fn current_actor(item: &WorkItem) -> String {
    item.assignee
        .clone()
        .unwrap_or_else(|| "agent:unknown".to_owned())
}

fn push_aid_task(item: &mut WorkItem, task_id: &str) {
    if !item.aid_task_ids.iter().any(|existing| existing == task_id) {
        item.aid_task_ids.push(task_id.to_owned());
    }
}

fn map_missing(error: anyhow::Error, id: &str) -> ApiError {
    if error.to_string() == "missing" {
        missing_item(id)
    } else {
        ApiError::internal(error)
    }
}
