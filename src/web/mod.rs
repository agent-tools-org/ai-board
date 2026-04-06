// Embedded static asset serving for the Axum dashboard.
// Exports: static_handler; Deps: axum, rust-embed.

use axum::{
    http::{Uri, header},
    response::IntoResponse,
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "web/static/"]
struct StaticAssets;

pub async fn static_handler(uri: Uri) -> impl IntoResponse {
    let requested = match uri.path().trim_start_matches('/') {
        "" => "index.html",
        path => path,
    };
    let asset = StaticAssets::get(requested)
        .map(|file| (requested, file))
        .or_else(|| StaticAssets::get("index.html").map(|file| ("index.html", file)));

    match asset {
        Some((path, file)) => (
            [(header::CONTENT_TYPE, content_type(path))],
            file.data.into_owned(),
        )
            .into_response(),
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}
