// Artifact API endpoints for listing, creating, and deleting item documents.
// Exports: routes() mounted under /api/items/{id}/artifacts using store helpers.

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{delete, get},
};
use chrono::Local;
use serde::Deserialize;
use serde_json::json;

use crate::{
    api::{ApiError, AppState, missing_item},
    store::{
        delete_artifact, gen_artifact_id, get_artifact, get_item, insert_artifact, list_artifacts,
    },
    types::{Artifact, ArtifactType},
};

const HUMAN_ACTOR: &str = "human:web";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/items/{id}/artifacts", get(list).post(create))
        .route("/api/items/{id}/artifacts/{artifact_id}", delete(remove))
}

#[derive(Deserialize)]
struct CreateArtifactBody {
    artifact_type: ArtifactType,
    title: String,
    path: Option<String>,
    content: Option<String>,
    status: Option<String>,
    created_by: Option<String>,
}

async fn list(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Artifact>>, ApiError> {
    store
        .with_connection(|conn| {
            ensure_item(conn, &id)?;
            list_artifacts(conn, &id)
        })
        .map(Json)
        .map_err(|error| map_missing(error, &id))
}

async fn create(
    State(store): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<CreateArtifactBody>,
) -> Result<Json<Artifact>, ApiError> {
    if body.title.trim().is_empty() {
        return Err(ApiError::bad_request("artifact title cannot be empty"));
    }
    let now = Local::now();
    let artifact = Artifact {
        id: gen_artifact_id(),
        item_id: id.clone(),
        artifact_type: body.artifact_type,
        title: body.title,
        path: body.path,
        content: body.content.unwrap_or_default(),
        status: body.status.unwrap_or_else(|| "final".to_owned()),
        created_by: body.created_by.unwrap_or_else(|| HUMAN_ACTOR.to_owned()),
        created_at: now,
        updated_at: now,
    };
    store
        .with_connection(|conn| {
            ensure_item(conn, &id)?;
            insert_artifact(conn, &artifact)?;
            Ok(artifact.clone())
        })
        .map(Json)
        .map_err(|error| map_missing(error, &id))
}

async fn remove(
    State(store): State<AppState>,
    Path((id, artifact_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    store
        .with_connection(|conn| {
            ensure_item(conn, &id)?;
            let artifact = get_artifact(conn, &artifact_id)?.ok_or_else(|| anyhow::anyhow!("missing-artifact"))?;
            if artifact.item_id != id {
                return Err(anyhow::anyhow!("missing-artifact"));
            }
            delete_artifact(conn, &artifact_id)?;
            Ok(Json(json!({ "deleted": true, "id": artifact_id })))
        })
        .map_err(|error| map_artifact_error(error, &id, &artifact_id))
}

fn ensure_item(conn: &rusqlite::Connection, id: &str) -> anyhow::Result<()> {
    get_item(conn, id)?
        .map(|_| ())
        .ok_or_else(|| anyhow::anyhow!("missing-item"))
}

fn map_missing(error: anyhow::Error, id: &str) -> ApiError {
    if error.to_string() == "missing-item" {
        missing_item(id)
    } else {
        ApiError::internal(error)
    }
}

fn map_artifact_error(error: anyhow::Error, item_id: &str, artifact_id: &str) -> ApiError {
    match error.to_string().as_str() {
        "missing-item" => missing_item(item_id),
        "missing-artifact" => ApiError::not_found(format!("artifact {artifact_id} not found")),
        _ => ApiError::internal(error),
    }
}
