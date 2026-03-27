//! REST API routes.

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, patch, post},
};
use serde::{Deserialize, Serialize};

use amigo_core::coordinator::Coordinator;
use amigo_core::queue::QueueStatus;

pub type AppState = Arc<Coordinator>;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/status", get(status))
        .route("/api/v1/stats", get(stats))
        .route("/api/v1/downloads", post(add_download))
        .route("/api/v1/downloads", get(list_downloads))
        .route("/api/v1/downloads/{id}", get(get_download))
        .route("/api/v1/downloads/{id}", patch(update_download))
        .route("/api/v1/downloads/{id}", delete(delete_download))
        .route("/api/v1/downloads/batch", post(add_batch))
        .route("/api/v1/queue", get(get_queue))
        .route("/api/v1/history", get(get_history))
        .with_state(state)
}

// --- Request / Response types ---

#[derive(Deserialize)]
struct AddDownloadRequest {
    url: String,
    filename: Option<String>,
}

#[derive(Deserialize)]
struct BatchRequest {
    urls: Vec<String>,
}

#[derive(Deserialize)]
struct UpdateDownloadRequest {
    action: String, // "pause", "resume"
}

#[derive(Serialize)]
struct StatusResponse {
    status: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct StatsResponse {
    active_downloads: usize,
    speed_bytes_per_sec: u64,
    queued: u32,
    completed: u32,
}

#[derive(Serialize)]
struct AddResponse {
    id: String,
}

#[derive(Serialize)]
struct BatchResponse {
    ids: Vec<String>,
}

#[derive(Serialize)]
struct DownloadResponse {
    id: String,
    url: String,
    protocol: String,
    filename: Option<String>,
    filesize: Option<u64>,
    status: String,
    priority: i32,
    bytes_downloaded: u64,
    speed: u64,
    error: Option<String>,
    created_at: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

// --- Handlers ---

async fn status() -> Json<StatusResponse> {
    Json(StatusResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn stats(State(coord): State<AppState>) -> Json<StatsResponse> {
    let active = coord.active_count().await;
    let speed = coord.total_speed().await;
    let queued = coord.storage().count_by_status(QueueStatus::Queued).await.unwrap_or(0);
    let completed = coord.storage().count_by_status(QueueStatus::Completed).await.unwrap_or(0);

    Json(StatsResponse {
        active_downloads: active,
        speed_bytes_per_sec: speed,
        queued,
        completed,
    })
}

async fn add_download(
    State(coord): State<AppState>,
    Json(req): Json<AddDownloadRequest>,
) -> Result<(StatusCode, Json<AddResponse>), (StatusCode, Json<ErrorResponse>)> {
    match coord.add_download(&req.url, req.filename).await {
        Ok(id) => Ok((StatusCode::CREATED, Json(AddResponse { id }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )),
    }
}

async fn add_batch(
    State(coord): State<AppState>,
    Json(req): Json<BatchRequest>,
) -> Result<(StatusCode, Json<BatchResponse>), (StatusCode, Json<ErrorResponse>)> {
    let mut ids = Vec::new();
    for url in &req.urls {
        match coord.add_download(url, None).await {
            Ok(id) => ids.push(id),
            Err(e) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse { error: e.to_string() }),
                ))
            }
        }
    }
    Ok((StatusCode::CREATED, Json(BatchResponse { ids })))
}

async fn list_downloads(State(coord): State<AppState>) -> Json<Vec<DownloadResponse>> {
    let rows = coord.storage().list_downloads().await.unwrap_or_default();
    Json(rows.into_iter().map(row_to_response).collect())
}

async fn get_download(
    State(coord): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DownloadResponse>, StatusCode> {
    match coord.storage().get_download(&id).await {
        Ok(Some(row)) => Ok(Json(row_to_response(row))),
        _ => Err(StatusCode::NOT_FOUND),
    }
}

async fn update_download(
    State(coord): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateDownloadRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let result = match req.action.as_str() {
        "pause" => coord.pause(&id).await,
        "resume" => coord.resume(&id).await,
        _ => return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "Invalid action. Use 'pause' or 'resume'.".into() }),
        )),
    };

    match result {
        Ok(()) => Ok(StatusCode::OK),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )),
    }
}

async fn delete_download(
    State(coord): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    match coord.cancel(&id).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )),
    }
}

async fn get_queue(State(coord): State<AppState>) -> Json<Vec<DownloadResponse>> {
    let rows = coord
        .storage()
        .list_downloads_by_status(QueueStatus::Queued)
        .await
        .unwrap_or_default();
    Json(rows.into_iter().map(row_to_response).collect())
}

async fn get_history(State(coord): State<AppState>) -> Json<Vec<DownloadResponse>> {
    let rows = coord.storage().get_history().await.unwrap_or_default();
    Json(rows.into_iter().map(row_to_response).collect())
}

fn row_to_response(row: amigo_core::storage::DownloadRow) -> DownloadResponse {
    DownloadResponse {
        id: row.id,
        url: row.url,
        protocol: row.protocol,
        filename: row.filename,
        filesize: row.filesize,
        status: row.status,
        priority: row.priority,
        bytes_downloaded: row.bytes_downloaded,
        speed: row.speed_current,
        error: row.error_message,
        created_at: row.created_at,
    }
}
