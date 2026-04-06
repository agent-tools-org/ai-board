// Axum API router setup for dashboard, agent, and SSE endpoints.
// Exports: router(), ApiError, AppState, and request helper functions.

pub mod agent;
pub mod artifacts;
pub mod items;
pub mod sse;

use std::sync::Arc;

use axum::{
    Json, Router,
    http::{HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Serialize;
use tower_http::cors::{Any, AllowOrigin, CorsLayer};

use crate::store::Store;
use crate::web::static_handler;

pub type AppState = Arc<Store>;

pub fn router(store: Arc<Store>) -> Router {
    Router::new()
        .merge(artifacts::routes())
        .merge(items::routes())
        .merge(agent::routes())
        .route("/api/stream", get(sse::stream))
        .fallback(static_handler)
        .layer(cors_layer())
        .with_state(store)
}

pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    pub fn internal(error: anyhow::Error) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self { status, message: message.into() }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(ErrorBody { error: self.message })).into_response()
    }
}

pub fn default_project(project: Option<String>) -> Option<String> {
    project.filter(|value| !value.is_empty())
}

pub fn default_repo_path(repo_path: Option<String>) -> Option<String> {
    repo_path.filter(|value| !value.is_empty())
}

pub fn agent_actor(name: &str) -> Result<String, ApiError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request("agent name cannot be empty"));
    }
    Ok(if trimmed.starts_with("agent:") {
        trimmed.to_owned()
    } else {
        format!("agent:{trimmed}")
    })
}

pub fn missing_item(id: &str) -> ApiError {
    ApiError::not_found(format!("item {id} not found"))
}

fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers(Any)
        .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
            origin
                .to_str()
                .ok()
                .is_some_and(|value| is_localhost_origin(value))
        }))
}

fn is_localhost_origin(value: &str) -> bool {
    value.starts_with("http://localhost:")
        || value.starts_with("https://localhost:")
        || value.starts_with("http://127.0.0.1:")
        || value.starts_with("https://127.0.0.1:")
        || value == "http://localhost"
        || value == "http://127.0.0.1"
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}
