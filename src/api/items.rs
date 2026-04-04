// Human dashboard item endpoints for CRUD, review actions, and ordering.
// Exports: routes() and handlers under /api/items using crate::store helpers.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch, post},
};
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    api::{ApiError, AppState, agent_actor, default_repo_path, missing_item},
    store::{ItemFilter, ItemUpdate, delete_item, gen_id, get_item, insert_event, insert_item, list_events, list_items, reorder_items, update_item, update_item_status},
    types::{Event, Priority, Status, WorkItem},
};

const HUMAN_ACTOR: &str = "human:web";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/items", get(list).post(create))
        .route("/api/items/reorder", patch(reorder))
        .route("/api/items/{id}", get(show).patch(update).delete(remove))
        .route("/api/items/{id}/approve", post(approve))
        .route("/api/items/{id}/reject", post(reject))
}

#[derive(Deserialize)]
struct ItemQuery {
    status: Option<Status>,
    priority: Option<Priority>,
    label: Option<String>,
    assignee: Option<String>,
    limit: Option<usize>,
    repo_path: Option<String>,
}

#[derive(Deserialize)]
struct CreateItemBody {
    repo_path: Option<String>,
    title: String,
    description: Option<String>,
    status: Option<Status>,
    priority: Option<Priority>,
    position: Option<f64>,
    labels: Option<Vec<String>>,
    parent_id: Option<String>,
    depends_on: Option<Vec<String>>,
    aid_task_ids: Option<Vec<String>>,
    aid_agent: Option<String>,
    aid_verify: Option<String>,
    estimate: Option<String>,
    assignee: Option<String>,
    created_by: Option<String>,
    requires_approval: Option<bool>,
    auto_dispatch: Option<bool>,
    due_date: Option<String>,
}

#[derive(Deserialize, Default)]
struct UpdateItemBody {
    repo_path: Option<String>,
    title: Option<String>,
    description: Option<String>,
    status: Option<Status>,
    priority: Option<Priority>,
    position: Option<f64>,
    labels: Option<Vec<String>>,
    parent_id: Option<Option<String>>,
    depends_on: Option<Vec<String>>,
    aid_task_ids: Option<Vec<String>>,
    aid_agent: Option<Option<String>>,
    aid_verify: Option<Option<String>>,
    estimate: Option<Option<String>>,
    assignee: Option<Option<String>>,
    requires_approval: Option<bool>,
    auto_dispatch: Option<bool>,
    due_date: Option<Option<String>>,
}

#[derive(Deserialize)]
struct RejectBody {
    feedback: Option<String>,
}

#[derive(Deserialize)]
struct ReorderInput {
    id: String,
    position: f64,
    status: Status,
}

#[derive(Serialize)]
struct ItemWithEvents {
    item: WorkItem,
    events: Vec<Event>,
}

async fn list(
    State(store): State<AppState>,
    Query(query): Query<ItemQuery>,
) -> Result<Json<Vec<WorkItem>>, ApiError> {
    let repo_path = default_repo_path(query.repo_path)?;
    let filter = ItemFilter {
        status: query.status,
        priority: query.priority,
        label: query.label,
        assignee: query.assignee,
        parent_id: None,
        repo_path: Some(repo_path),
        limit: query.limit,
    };
    let items = store.with_connection(|conn| list_items(conn, &filter)).map_err(ApiError::internal)?;
    Ok(Json(items))
}

async fn show(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ItemWithEvents>, ApiError> {
    store
        .with_connection(|conn| {
            let item = get_item(conn, &id)?.ok_or_else(|| anyhow::anyhow!("missing"))?;
            let events = list_events(conn, &id, None)?;
            Ok(Json(ItemWithEvents { item, events }))
        })
        .map_err(|error| if error.to_string() == "missing" { missing_item(&id) } else { ApiError::internal(error) })
}

async fn create(
    State(store): State<AppState>,
    Query(query): Query<ItemQuery>,
    Json(body): Json<CreateItemBody>,
) -> Result<Json<WorkItem>, ApiError> {
    if body.title.trim().is_empty() {
        return Err(ApiError::bad_request("title cannot be empty"));
    }
    let now = Local::now();
    let item = WorkItem {
        id: gen_id(),
        repo_path: default_repo_path(body.repo_path.or(query.repo_path))?,
        title: body.title,
        description: body.description.unwrap_or_default(),
        status: body.status.unwrap_or(Status::Backlog),
        priority: body.priority.unwrap_or(Priority::Medium),
        position: body.position.unwrap_or(0.0),
        labels: body.labels.unwrap_or_default(),
        parent_id: body.parent_id,
        depends_on: body.depends_on.unwrap_or_default(),
        aid_task_ids: body.aid_task_ids.unwrap_or_default(),
        aid_agent: body.aid_agent,
        aid_verify: body.aid_verify,
        estimate: body.estimate,
        assignee: normalize_optional_assignee(body.assignee)?,
        created_by: body.created_by.unwrap_or_else(|| HUMAN_ACTOR.to_owned()),
        requires_approval: body.requires_approval.unwrap_or(false),
        auto_dispatch: body.auto_dispatch.unwrap_or(false),
        created_at: now,
        updated_at: now,
        started_at: None,
        completed_at: None,
        due_date: body.due_date,
    };
    store
        .with_connection(|conn| {
            insert_item(conn, &item)?;
            insert_event(conn, &item.id, HUMAN_ACTOR, "created", Some(&item.title), None)?;
            Ok(Json(item.clone()))
        })
        .map_err(ApiError::internal)
}

async fn update(
    State(store): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateItemBody>,
) -> Result<Json<WorkItem>, ApiError> {
    let update = ItemUpdate {
        repo_path: body.repo_path,
        title: body.title,
        description: body.description,
        priority: body.priority,
        position: body.position,
        labels: body.labels,
        parent_id: body.parent_id,
        depends_on: body.depends_on,
        aid_task_ids: body.aid_task_ids,
        aid_agent: body.aid_agent,
        aid_verify: body.aid_verify,
        estimate: body.estimate,
        assignee: normalize_update_assignee(body.assignee)?,
        requires_approval: body.requires_approval,
        auto_dispatch: body.auto_dispatch,
        due_date: body.due_date,
    };
    store
        .with_connection(|conn| {
            ensure_exists(conn, &id)?;
            update_item(conn, &id, &update)?;
            if let Some(status) = body.status {
                update_item_status(conn, &id, status)?;
            }
            insert_event(conn, &id, HUMAN_ACTOR, "updated", None, None)?;
            get_item(conn, &id)?.ok_or_else(|| anyhow::anyhow!("missing"))
        })
        .map(Json)
        .map_err(|error| if error.to_string() == "missing" { missing_item(&id) } else { ApiError::internal(error) })
}

async fn remove(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    store
        .with_connection(|conn| {
            ensure_exists(conn, &id)?;
            delete_item(conn, &id)?;
            Ok(Json(json!({ "deleted": true, "id": id })))
        })
        .map_err(|error| if error.to_string() == "missing" { missing_item(&id) } else { ApiError::internal(error) })
}

async fn approve(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<WorkItem>, ApiError> {
    transition_item(store, id, Status::Done, "approved", None, None).await
}

async fn reject(
    State(store): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<RejectBody>,
) -> Result<Json<WorkItem>, ApiError> {
    transition_item(store, id, Status::Ready, "rejected", body.feedback.as_deref(), None).await
}

async fn reorder(
    State(store): State<AppState>,
    Json(body): Json<Vec<ReorderInput>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let updates = body
        .iter()
        .map(|entry| {
            serde_json::to_string(&entry.status)
                .map(|status| (entry.id.clone(), entry.position, status.trim_matches('"').to_owned()))
                .map_err(|error| ApiError::internal(error.into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let count = updates.len();
    store.with_connection(|conn| reorder_items(conn, &updates)).map_err(ApiError::internal)?;
    Ok(Json(json!({ "updated": count })))
}

async fn transition_item(
    store: AppState,
    id: String,
    status: Status,
    action: &str,
    detail: Option<&str>,
    metadata: Option<serde_json::Value>,
) -> Result<Json<WorkItem>, ApiError> {
    store
        .with_connection(|conn| {
            ensure_exists(conn, &id)?;
            update_item_status(conn, &id, status)?;
            insert_event(conn, &id, HUMAN_ACTOR, action, detail, metadata.as_ref())?;
            get_item(conn, &id)?.ok_or_else(|| anyhow::anyhow!("missing"))
        })
        .map(Json)
        .map_err(|error| if error.to_string() == "missing" { missing_item(&id) } else { ApiError::internal(error) })
}

fn ensure_exists(conn: &rusqlite::Connection, id: &str) -> anyhow::Result<()> {
    get_item(conn, id)?
        .map(|_| ())
        .ok_or_else(|| anyhow::anyhow!("missing"))
}

fn normalize_optional_assignee(assignee: Option<String>) -> Result<Option<String>, ApiError> {
    assignee.map(|value| agent_actor(&value)).transpose()
}

fn normalize_update_assignee(
    assignee: Option<Option<String>>,
) -> Result<Option<Option<String>>, ApiError> {
    assignee.map(|value| value.map(|inner| agent_actor(&inner)).transpose()).transpose()
}
