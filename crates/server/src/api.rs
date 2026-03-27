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
use amigo_plugin_runtime::loader::PluginLoader;
use amigo_plugin_runtime::updater::PluginUpdater;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub coordinator: Arc<Coordinator>,
    pub plugins: Arc<PluginLoader>,
    pub plugin_updater: Arc<PluginUpdater>,
    pub http_client: reqwest::Client,
}

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
        // Usenet endpoints
        .route("/api/v1/downloads/nzb", post(upload_nzb))
        .route("/api/v1/usenet/servers", get(list_usenet_servers))
        .route("/api/v1/usenet/servers", post(add_usenet_server))
        .route("/api/v1/usenet/servers/{id}", delete(delete_usenet_server))
        // Plugin endpoints
        .route("/api/v1/plugins", get(list_plugins))
        .route("/api/v1/plugins/{id}", patch(update_plugin))
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

#[derive(Deserialize)]
struct UpdatePluginRequest {
    enabled: Option<bool>,
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
struct PluginResponse {
    id: String,
    name: String,
    version: String,
    url_pattern: String,
    enabled: bool,
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

async fn stats(State(state): State<AppState>) -> Json<StatsResponse> {
    let active = state.coordinator.active_count().await;
    let speed = state.coordinator.total_speed().await;
    let queued = state
        .coordinator
        .storage()
        .count_by_status(QueueStatus::Queued)
        .await
        .unwrap_or(0);
    let completed = state
        .coordinator
        .storage()
        .count_by_status(QueueStatus::Completed)
        .await
        .unwrap_or(0);

    Json(StatsResponse {
        active_downloads: active,
        speed_bytes_per_sec: speed,
        queued,
        completed,
    })
}

async fn add_download(
    State(state): State<AppState>,
    Json(req): Json<AddDownloadRequest>,
) -> Result<(StatusCode, Json<AddResponse>), (StatusCode, Json<ErrorResponse>)> {
    match state.coordinator.add_download(&req.url, req.filename).await {
        Ok(id) => Ok((StatusCode::CREATED, Json(AddResponse { id }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

async fn add_batch(
    State(state): State<AppState>,
    Json(req): Json<BatchRequest>,
) -> Result<(StatusCode, Json<BatchResponse>), (StatusCode, Json<ErrorResponse>)> {
    let mut ids = Vec::new();
    for url in &req.urls {
        match state.coordinator.add_download(url, None).await {
            Ok(id) => ids.push(id),
            Err(e) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: e.to_string(),
                    }),
                ));
            }
        }
    }
    Ok((StatusCode::CREATED, Json(BatchResponse { ids })))
}

async fn list_downloads(State(state): State<AppState>) -> Json<Vec<DownloadResponse>> {
    let rows = state
        .coordinator
        .storage()
        .list_downloads()
        .await
        .unwrap_or_default();
    Json(rows.into_iter().map(row_to_response).collect())
}

async fn get_download(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DownloadResponse>, StatusCode> {
    match state.coordinator.storage().get_download(&id).await {
        Ok(Some(row)) => Ok(Json(row_to_response(row))),
        _ => Err(StatusCode::NOT_FOUND),
    }
}

async fn update_download(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateDownloadRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let result = match req.action.as_str() {
        "pause" => state.coordinator.pause(&id).await,
        "resume" => state.coordinator.resume(&id).await,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid action. Use 'pause' or 'resume'.".into(),
                }),
            ));
        }
    };

    match result {
        Ok(()) => Ok(StatusCode::OK),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

async fn delete_download(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    match state.coordinator.cancel(&id).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

async fn get_queue(State(state): State<AppState>) -> Json<Vec<DownloadResponse>> {
    let rows = state
        .coordinator
        .storage()
        .list_downloads_by_status(QueueStatus::Queued)
        .await
        .unwrap_or_default();
    Json(rows.into_iter().map(row_to_response).collect())
}

async fn get_history(State(state): State<AppState>) -> Json<Vec<DownloadResponse>> {
    let rows = state
        .coordinator
        .storage()
        .get_history()
        .await
        .unwrap_or_default();
    Json(rows.into_iter().map(row_to_response).collect())
}

// --- Plugin handlers ---

async fn list_plugins(State(state): State<AppState>) -> Json<Vec<PluginResponse>> {
    let plugins = state.plugins.list_plugins().await;
    Json(
        plugins
            .into_iter()
            .map(|p| PluginResponse {
                id: p.id,
                name: p.name,
                version: p.version,
                url_pattern: p.url_pattern,
                enabled: p.enabled,
            })
            .collect(),
    )
}

async fn update_plugin(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdatePluginRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    if let Some(enabled) = req.enabled {
        state.plugins.set_enabled(&id, enabled).await.map_err(|e| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;
    }
    Ok(StatusCode::OK)
}

// --- Usenet handlers ---

#[derive(Deserialize)]
struct NzbUploadRequest {
    nzb_data: String,
}

#[derive(Deserialize)]
struct AddUsenetServerRequest {
    name: String,
    host: String,
    port: u16,
    ssl: bool,
    username: String,
    password: String,
    connections: u32,
    priority: u32,
}

#[derive(Serialize)]
struct UsenetServerResponse {
    id: String,
    name: String,
    host: String,
    port: u16,
    ssl: bool,
    connections: u32,
    priority: u32,
}

async fn upload_nzb(
    State(state): State<AppState>,
    Json(req): Json<NzbUploadRequest>,
) -> Result<(StatusCode, Json<AddResponse>), (StatusCode, Json<ErrorResponse>)> {
    // Parse NZB to validate it
    amigo_core::protocol::usenet::nzb::parse_nzb(&req.nzb_data).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid NZB: {e}"),
            }),
        )
    })?;

    // Add as a download with usenet protocol
    match state
        .coordinator
        .add_download("nzb://upload", Some("nzb-import".into()))
        .await
    {
        Ok(id) => Ok((StatusCode::CREATED, Json(AddResponse { id }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

async fn list_usenet_servers() -> Json<Vec<UsenetServerResponse>> {
    // TODO: Load from config/database
    Json(Vec::new())
}

async fn add_usenet_server(
    Json(_req): Json<AddUsenetServerRequest>,
) -> Result<(StatusCode, Json<UsenetServerResponse>), (StatusCode, Json<ErrorResponse>)> {
    // TODO: Persist to config/database
    Err((
        StatusCode::NOT_IMPLEMENTED,
        Json(ErrorResponse {
            error: "Not yet implemented".into(),
        }),
    ))
}

async fn delete_usenet_server(
    Path(_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    // TODO: Remove from config/database
    Err((
        StatusCode::NOT_IMPLEMENTED,
        Json(ErrorResponse {
            error: "Not yet implemented".into(),
        }),
    ))
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
