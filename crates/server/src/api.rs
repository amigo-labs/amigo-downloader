//! REST API routes.

use axum::{Router, routing::get};

pub fn router() -> Router {
    Router::new()
        .route("/api/v1/status", get(status))
        .route("/api/v1/stats", get(stats))
    // TODO: downloads, queue, plugins, usenet, torrent, history, config endpoints
    // TODO: POST /api/v1/downloads/container for DLC import
    // TODO: GET /api/v1/downloads/export/dlc for DLC export
}

async fn status() -> &'static str {
    r#"{"status":"ok","version":"0.1.0"}"#
}

async fn stats() -> &'static str {
    r#"{"active_downloads":0,"speed":0,"queue_size":0}"#
}
