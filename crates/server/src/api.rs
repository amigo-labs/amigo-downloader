//! REST API routes.

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::StatusCode,
    routing::{delete, get, patch, post, put},
};
use serde::{Deserialize, Serialize};

use amigo_core::captcha::CaptchaManager;
use amigo_core::coordinator::Coordinator;
use amigo_core::queue::QueueStatus;
use amigo_plugin_runtime::loader::PluginLoader;
use amigo_plugin_runtime::updater::PluginUpdater;

use crate::webhooks::WebhookDispatcher;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub coordinator: Arc<Coordinator>,
    pub plugins: Arc<PluginLoader>,
    pub plugin_updater: Arc<PluginUpdater>,
    pub http_client: reqwest::Client,
    pub captcha_manager: Arc<CaptchaManager>,
    pub webhook_dispatcher: Arc<WebhookDispatcher>,
    pub config_path: std::path::PathBuf,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/status", get(status))
        .route("/api/v1/stats", get(stats))
        .route("/api/v1/downloads", post(add_download))
        .route("/api/v1/downloads", get(list_downloads))
        // Specific download routes MUST come before {id} routes
        .route("/api/v1/downloads/batch", post(add_batch))
        // NZB JSON bodies can be a few MiB for large releases; cap at 64 MiB
        // so a malicious client can't exhaust memory by uploading a multi-GiB
        // body. DLC containers are tiny (tens of KiB at most), so cap them
        // at 1 MiB. Both overrides come from the audit's CRITICAL #3 finding
        // — without them axum's default 2 MiB applied to one route and the
        // multipart handler on the other could buffer arbitrary sizes.
        .route(
            "/api/v1/downloads/nzb",
            post(upload_nzb).layer(DefaultBodyLimit::max(MAX_NZB_BODY_BYTES)),
        )
        .route(
            "/api/v1/downloads/container",
            post(upload_container).layer(DefaultBodyLimit::max(MAX_DLC_BODY_BYTES)),
        )
        .route("/api/v1/downloads/usenet", get(list_usenet_downloads))
        .route("/api/v1/downloads/{id}", get(get_download))
        .route("/api/v1/downloads/{id}", patch(update_download))
        .route("/api/v1/downloads/{id}", delete(delete_download))
        .route("/api/v1/queue", get(get_queue))
        .route("/api/v1/queue/reorder", patch(reorder_queue))
        .route("/api/v1/history", get(get_history))
        .route("/api/v1/history", delete(delete_history))
        // Usenet endpoints
        .route("/api/v1/usenet/servers", get(list_usenet_servers))
        .route("/api/v1/usenet/servers", post(add_usenet_server))
        .route("/api/v1/usenet/servers/{id}", delete(delete_usenet_server))
        .route("/api/v1/usenet/watch-dir", get(get_nzb_watch_dir))
        .route("/api/v1/usenet/watch-dir", post(set_nzb_watch_dir))
        // Unified config endpoint
        .route("/api/v1/config", get(get_config))
        .route("/api/v1/config", put(put_config))
        // RSS feed endpoints (feature-gated in handlers)
        .route("/api/v1/rss", get(list_rss_feeds))
        .route("/api/v1/rss", post(add_rss_feed))
        .route("/api/v1/rss/{id}", delete(delete_rss_feed))
        // Plugin endpoints — specific routes before {id}
        .route("/api/v1/plugins", get(list_plugins))
        .route("/api/v1/plugins/suggest", post(suggest_plugin))
        .route("/api/v1/plugins/{id}", patch(update_plugin))
        // Captcha endpoints — specific routes before {id}
        .route("/api/v1/captcha/pending", get(list_pending_captchas))
        .route("/api/v1/captcha/{id}/solve", post(solve_captcha))
        .route("/api/v1/captcha/{id}/cancel", post(cancel_captcha))
        // Webhook endpoints
        .route("/api/v1/webhooks", get(list_webhooks))
        .route("/api/v1/webhooks", post(create_webhook))
        .route("/api/v1/webhooks/{id}", delete(delete_webhook))
        .route("/api/v1/webhooks/{id}/test", post(test_webhook))
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
    errors: Vec<String>,
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
) -> (StatusCode, Json<BatchResponse>) {
    let mut ids = Vec::new();
    let mut errors = Vec::new();
    for url in &req.urls {
        match state.coordinator.add_download(url, None).await {
            Ok(id) => ids.push(id),
            Err(e) => errors.push(format!("{url}: {e}")),
        }
    }
    let status = if !errors.is_empty() {
        StatusCode::MULTI_STATUS
    } else {
        StatusCode::CREATED
    };
    (status, Json(BatchResponse { ids, errors }))
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

#[derive(Deserialize)]
struct ReorderRequest {
    /// Download IDs in desired order. Priority is set based on position.
    ids: Vec<String>,
}

async fn reorder_queue(
    State(state): State<AppState>,
    Json(req): Json<ReorderRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    for (i, id) in req.ids.iter().enumerate() {
        // Higher position index = lower priority (executed later)
        let priority = -(i as i32);
        state
            .coordinator
            .set_priority(id, priority)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: e.to_string(),
                    }),
                )
            })?;
    }
    Ok(StatusCode::OK)
}

async fn delete_history(
    State(state): State<AppState>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state
        .coordinator
        .storage()
        .clear_history()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;
    Ok(StatusCode::NO_CONTENT)
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

// --- Plugin suggestion ---

#[derive(Deserialize)]
struct SuggestPluginRequest {
    url: String,
}

#[derive(Serialize)]
struct SuggestPluginResponse {
    found: bool,
    plugin_id: Option<String>,
    plugin_name: Option<String>,
    install_command: Option<String>,
}

async fn suggest_plugin(
    State(state): State<AppState>,
    Json(req): Json<SuggestPluginRequest>,
) -> Json<SuggestPluginResponse> {
    let config = amigo_plugin_runtime::registry::RegistryConfig::default();
    let index = amigo_plugin_runtime::registry::load_index(&state.http_client, &config).await;

    if let Ok(index) = index
        && let Some(plugin) =
            amigo_plugin_runtime::registry::suggest_plugin_for_url(&index, &req.url)
    {
        return Json(SuggestPluginResponse {
            found: true,
            plugin_id: Some(plugin.id.clone()),
            plugin_name: Some(plugin.name.clone()),
            install_command: Some(format!("amigo-dl plugins install {}", plugin.id)),
        });
    }

    Json(SuggestPluginResponse {
        found: false,
        plugin_id: None,
        plugin_name: None,
        install_command: None,
    })
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

#[derive(Deserialize)]
struct SetWatchDirRequest {
    path: String,
}

#[derive(Serialize)]
struct WatchDirResponse {
    path: String,
}

async fn upload_nzb(
    State(state): State<AppState>,
    Json(req): Json<NzbUploadRequest>,
) -> Result<(StatusCode, Json<AddResponse>), (StatusCode, Json<ErrorResponse>)> {
    if !state.coordinator.config().await.features.usenet {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "Usenet feature is disabled".into(),
            }),
        ));
    }

    let nzb = amigo_core::protocol::usenet::nzb::parse_nzb(&req.nzb_data).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid NZB: {e}"),
            }),
        )
    })?;

    let file_count = nzb.files.len();
    let total_bytes: u64 = nzb.files.iter().map(|f| f.total_bytes()).sum();
    let first_name = nzb
        .files
        .first()
        .map(|f| f.filename())
        .unwrap_or_else(|| "nzb-import".into());

    match state
        .coordinator
        .add_download("nzb://upload", Some(first_name))
        .await
    {
        Ok(id) => {
            // Store NZB data + metadata so the coordinator can download segments later
            let metadata = serde_json::json!({
                "file_count": file_count,
                "total_bytes": total_bytes,
                "nzb_data": req.nzb_data,
            });
            let _ = state
                .coordinator
                .storage()
                .update_download_metadata(&id, &metadata.to_string())
                .await;

            // Also set filesize from NZB total
            let _ = state
                .coordinator
                .storage()
                .update_download_progress(&id, 0, 0)
                .await;

            tracing::info!("NZB imported: {id} ({file_count} files, {total_bytes} bytes)");
            Ok((StatusCode::CREATED, Json(AddResponse { id })))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

#[derive(Serialize)]
struct ContainerImportResponse {
    packages: usize,
    added: Vec<String>,
}

async fn upload_container(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<ContainerImportResponse>), (StatusCode, Json<ErrorResponse>)> {
    let mut data: Option<Vec<u8>> = None;
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid multipart body: {e}"),
            }),
        )
    })? {
        if field.name() == Some("file") {
            let bytes = field.bytes().await.map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!("Failed to read file field: {e}"),
                    }),
                )
            })?;
            data = Some(bytes.to_vec());
            break;
        }
    }

    let data = data.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Missing 'file' field in multipart body".into(),
            }),
        )
    })?;

    let packages = amigo_core::container::import_dlc(&data).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid DLC: {e}"),
            }),
        )
    })?;

    let mut added = Vec::new();
    for pkg in &packages {
        for link in &pkg.links {
            match state
                .coordinator
                .add_download(&link.url, link.filename.clone())
                .await
            {
                Ok(id) => added.push(id),
                Err(e) => {
                    tracing::warn!("DLC link {} failed to queue: {e}", link.url);
                }
            }
        }
    }

    tracing::info!(
        "DLC imported: {} package(s), {} download(s) queued",
        packages.len(),
        added.len()
    );

    Ok((
        StatusCode::CREATED,
        Json(ContainerImportResponse {
            packages: packages.len(),
            added,
        }),
    ))
}

async fn list_usenet_downloads(
    State(state): State<AppState>,
) -> Result<Json<Vec<DownloadResponse>>, (StatusCode, Json<ErrorResponse>)> {
    if !state.coordinator.config().await.features.usenet {
        return Ok(Json(Vec::new()));
    }

    let rows = state
        .coordinator
        .storage()
        .list_downloads_by_protocol("usenet")
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    let downloads = rows.into_iter().map(row_to_response).collect();
    Ok(Json(downloads))
}

async fn list_usenet_servers(
    State(state): State<AppState>,
) -> Result<Json<Vec<UsenetServerResponse>>, (StatusCode, Json<ErrorResponse>)> {
    if !state.coordinator.config().await.features.usenet {
        return Ok(Json(Vec::new()));
    }

    let rows = state
        .coordinator
        .storage()
        .list_usenet_servers()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    let servers = rows
        .into_iter()
        .map(|r| UsenetServerResponse {
            id: r.id,
            name: r.name,
            host: r.host,
            port: r.port,
            ssl: r.ssl,
            connections: r.connections,
            priority: r.priority,
        })
        .collect();
    Ok(Json(servers))
}

async fn add_usenet_server(
    State(state): State<AppState>,
    Json(req): Json<AddUsenetServerRequest>,
) -> Result<(StatusCode, Json<UsenetServerResponse>), (StatusCode, Json<ErrorResponse>)> {
    if !state.coordinator.config().await.features.usenet {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "Usenet feature is disabled".into(),
            }),
        ));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let row = amigo_core::storage::UsenetServerRow {
        id: id.clone(),
        name: req.name.clone(),
        host: req.host.clone(),
        port: req.port,
        ssl: req.ssl,
        username: req.username,
        password: req.password,
        connections: req.connections,
        priority: req.priority,
        created_at: String::new(),
    };

    state
        .coordinator
        .storage()
        .insert_usenet_server(&row)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok((
        StatusCode::CREATED,
        Json(UsenetServerResponse {
            id,
            name: req.name,
            host: req.host,
            port: req.port,
            ssl: req.ssl,
            connections: req.connections,
            priority: req.priority,
        }),
    ))
}

async fn delete_usenet_server(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state
        .coordinator
        .storage()
        .delete_usenet_server(&id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_nzb_watch_dir(
    State(state): State<AppState>,
) -> Result<Json<WatchDirResponse>, (StatusCode, Json<ErrorResponse>)> {
    let path = state
        .coordinator
        .storage()
        .get_update_state("nzb_watch_dir")
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?
        .unwrap_or_default();
    Ok(Json(WatchDirResponse { path }))
}

async fn set_nzb_watch_dir(
    State(state): State<AppState>,
    Json(req): Json<SetWatchDirRequest>,
) -> Result<Json<WatchDirResponse>, (StatusCode, Json<ErrorResponse>)> {
    state
        .coordinator
        .storage()
        .set_update_state("nzb_watch_dir", &req.path)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;
    Ok(Json(WatchDirResponse { path: req.path }))
}

// --- Body-size limits for upload endpoints (audit finding #3) ---

/// Maximum size of an NZB JSON body. Real-world NZBs grow with article count;
/// 64 MiB comfortably handles even multi-thousand-article releases while
/// blocking obvious DoS payloads.
const MAX_NZB_BODY_BYTES: usize = 64 * 1024 * 1024;

/// Maximum size of a DLC container upload. DLC files carry an AES-CBC-
/// encrypted XML link list; legitimate ones are kilobytes, never megabytes.
const MAX_DLC_BODY_BYTES: usize = 1024 * 1024;

// --- Unified config endpoint ---

/// Sentinel string substituted for secret fields in config responses. The UI
/// renders these as masked inputs ("••••") and round-trips them back on PUT;
/// `apply_secret_passthrough` preserves the on-disk value whenever the
/// sentinel arrives unchanged so non-secret edits don't accidentally wipe
/// credentials.
const REDACTED_SENTINEL: &str = "__redacted__";

/// Build a copy of `config` safe to return over the API. Plaintext secrets
/// (API tokens, GitHub PAT, NZBGet password, webhook signing secrets, the
/// admin password hash) are replaced with [`REDACTED_SENTINEL`] when set,
/// or left as `None`/empty when not configured.
fn redact_config(mut config: amigo_core::config::Config) -> amigo_core::config::Config {
    if config.server.api_token.is_some() {
        config.server.api_token = Some(REDACTED_SENTINEL.into());
    }
    if config.server.admin_password_hash.is_some() {
        config.server.admin_password_hash = Some(REDACTED_SENTINEL.into());
    }
    if !config.nzbget_api.password.is_empty() {
        config.nzbget_api.password = REDACTED_SENTINEL.into();
    }
    if !config.feedback.github_token.is_empty() {
        config.feedback.github_token = REDACTED_SENTINEL.into();
    }
    for hook in &mut config.webhooks {
        if hook.secret.is_some() {
            hook.secret = Some(REDACTED_SENTINEL.into());
        }
    }
    config
}

/// When a PUT request comes back with a secret field still equal to
/// [`REDACTED_SENTINEL`], that means the UI never saw the plaintext and is
/// just round-tripping the masked value — keep the existing on-disk secret
/// instead of overwriting it with the sentinel string.
fn apply_secret_passthrough(
    incoming: &mut amigo_core::config::Config,
    existing: &amigo_core::config::Config,
) {
    if incoming.server.api_token.as_deref() == Some(REDACTED_SENTINEL) {
        incoming.server.api_token = existing.server.api_token.clone();
    }
    if incoming.server.admin_password_hash.as_deref() == Some(REDACTED_SENTINEL) {
        incoming.server.admin_password_hash = existing.server.admin_password_hash.clone();
    }
    if incoming.nzbget_api.password == REDACTED_SENTINEL {
        incoming.nzbget_api.password = existing.nzbget_api.password.clone();
    }
    if incoming.feedback.github_token == REDACTED_SENTINEL {
        incoming.feedback.github_token = existing.feedback.github_token.clone();
    }
    // Webhooks are matched by id; if a hook with the same id already exists
    // and the incoming secret is the sentinel, restore the previous secret.
    for hook in &mut incoming.webhooks {
        if hook.secret.as_deref() == Some(REDACTED_SENTINEL) {
            hook.secret = existing
                .webhooks
                .iter()
                .find(|h| h.id == hook.id)
                .and_then(|h| h.secret.clone());
        }
    }
}

async fn get_config(State(state): State<AppState>) -> Json<amigo_core::config::Config> {
    let config = state.coordinator.config().await;
    Json(redact_config(config))
}

async fn put_config(
    State(state): State<AppState>,
    Json(mut new_config): Json<amigo_core::config::Config>,
) -> Result<Json<amigo_core::config::Config>, (StatusCode, Json<ErrorResponse>)> {
    // Restore secrets the UI never saw so a "save settings" round-trip from
    // a masked GET response cannot accidentally erase credentials.
    let existing = state.coordinator.config().await;
    apply_secret_passthrough(&mut new_config, &existing);

    // Save to TOML file
    new_config.save(&state.config_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    // Update runtime state (bandwidth limiter, retry policy, etc.)
    state.coordinator.update_config(new_config.clone()).await;

    tracing::info!("Config updated and saved to {:?}", state.config_path);
    Ok(Json(redact_config(new_config)))
}

// --- RSS feed handlers (feature-gated) ---

#[derive(Serialize)]
struct RssFeedResponse {
    id: String,
    name: String,
    url: String,
    category: String,
    interval_minutes: u32,
    enabled: bool,
    last_check: Option<String>,
    last_error: Option<String>,
}

#[derive(Deserialize)]
struct AddRssFeedRequest {
    name: String,
    url: String,
    #[serde(default)]
    category: String,
    #[serde(default = "default_rss_interval")]
    interval_minutes: u32,
}

fn default_rss_interval() -> u32 {
    15
}

async fn list_rss_feeds(
    State(state): State<AppState>,
) -> Result<Json<Vec<RssFeedResponse>>, (StatusCode, Json<ErrorResponse>)> {
    if !state.coordinator.config().await.features.rss_feeds {
        return Ok(Json(Vec::new()));
    }

    let rows = state
        .coordinator
        .storage()
        .list_rss_feeds()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok(Json(
        rows.into_iter()
            .map(|r| RssFeedResponse {
                id: r.id,
                name: r.name,
                url: r.url,
                category: r.category,
                interval_minutes: r.interval_minutes,
                enabled: r.enabled,
                last_check: r.last_check,
                last_error: r.last_error,
            })
            .collect(),
    ))
}

async fn add_rss_feed(
    State(state): State<AppState>,
    Json(req): Json<AddRssFeedRequest>,
) -> Result<(StatusCode, Json<RssFeedResponse>), (StatusCode, Json<ErrorResponse>)> {
    if !state.coordinator.config().await.features.rss_feeds {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "RSS feeds feature is disabled. Enable it in Settings.".into(),
            }),
        ));
    }

    // SSRF guard: refuse RSS feeds whose URL points at non-public targets
    // (loopback, RFC1918, link-local, cloud metadata, …) or non-http(s)
    // schemes. Without this, a privileged user could turn the feed poller
    // into an internal scanner.
    crate::net_guard::validate_outbound_url(&req.url, false)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("RSS feed URL rejected: {e}"),
                }),
            )
        })?;

    let id = uuid::Uuid::new_v4().to_string();
    let row = amigo_core::storage::RssFeedRow {
        id: id.clone(),
        name: req.name.clone(),
        url: req.url.clone(),
        category: req.category.clone(),
        interval_minutes: req.interval_minutes,
        enabled: true,
        last_check: None,
        last_error: None,
        created_at: String::new(),
    };

    state
        .coordinator
        .storage()
        .insert_rss_feed(&row)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok((
        StatusCode::CREATED,
        Json(RssFeedResponse {
            id,
            name: req.name,
            url: req.url,
            category: req.category,
            interval_minutes: req.interval_minutes,
            enabled: true,
            last_check: None,
            last_error: None,
        }),
    ))
}

async fn delete_rss_feed(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state
        .coordinator
        .storage()
        .delete_rss_feed(&id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;
    Ok(StatusCode::NO_CONTENT)
}

// --- Captcha handlers ---

async fn list_pending_captchas(State(state): State<AppState>) -> Json<serde_json::Value> {
    let pending = state.captcha_manager.list_pending().await;
    Json(serde_json::to_value(pending).unwrap_or_default())
}

#[derive(Deserialize)]
struct SolveCaptchaRequest {
    answer: String,
}

async fn solve_captcha(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<SolveCaptchaRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state
        .captcha_manager
        .submit_solution(&id, &req.answer)
        .await
        .map_err(|e| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;
    Ok(StatusCode::OK)
}

async fn cancel_captcha(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state.captcha_manager.cancel(&id).await.map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;
    Ok(StatusCode::OK)
}

// --- Webhook handlers ---

async fn list_webhooks(State(state): State<AppState>) -> Json<serde_json::Value> {
    let endpoints = state.webhook_dispatcher.list_endpoints().await;
    Json(serde_json::to_value(endpoints).unwrap_or_default())
}

#[derive(Deserialize)]
struct CreateWebhookRequest {
    name: String,
    url: String,
    secret: Option<String>,
    events: Option<Vec<String>>,
}

async fn create_webhook(
    State(state): State<AppState>,
    Json(req): Json<CreateWebhookRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<ErrorResponse>)> {
    // SSRF guard: webhook URLs are operator-supplied. Without validation, a
    // privileged user could register http://169.254.169.254/ (cloud metadata)
    // or http://10.0.0.1/admin and have the server probe / exfiltrate.
    crate::net_guard::validate_outbound_url(&req.url, false)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Webhook URL rejected: {e}"),
                }),
            )
        })?;

    let endpoint = amigo_core::config::WebhookEndpoint {
        id: uuid::Uuid::new_v4().to_string(),
        name: req.name,
        url: req.url,
        secret: req.secret,
        events: req.events.unwrap_or_else(|| vec!["*".into()]),
        enabled: true,
        retry_count: 3,
        retry_delay_secs: 10,
    };
    let resp = serde_json::to_value(&endpoint).unwrap_or_default();
    state.webhook_dispatcher.add_endpoint(endpoint).await;
    Ok((StatusCode::CREATED, Json(resp)))
}

async fn delete_webhook(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    if state.webhook_dispatcher.remove_endpoint(&id).await {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[derive(Serialize)]
struct TestWebhookResponse {
    status: u16,
}

async fn test_webhook(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TestWebhookResponse>, (StatusCode, Json<ErrorResponse>)> {
    match state.webhook_dispatcher.send_test(&id).await {
        Ok(status) => Ok(Json(TestWebhookResponse { status })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e }),
        )),
    }
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

#[cfg(test)]
mod config_redact_tests {
    use super::*;
    use amigo_core::config::{Config, WebhookEndpoint};

    fn config_with_secrets() -> Config {
        let mut cfg = Config::default();
        cfg.server.api_token = Some("ApiTokenAbc123".into());
        cfg.server.admin_password_hash = Some("$argon2id$v=19$...".into());
        cfg.nzbget_api.password = "nzbpass".into();
        cfg.feedback.github_token = "ghp_realtoken".into();
        cfg.webhooks.push(WebhookEndpoint {
            id: "hook-1".into(),
            name: "test".into(),
            url: "https://example.com".into(),
            secret: Some("hmacsecret".into()),
            events: vec!["*".into()],
            enabled: true,
            retry_count: 3,
            retry_delay_secs: 10,
        });
        cfg
    }

    #[test]
    fn redact_replaces_every_secret_field() {
        let redacted = redact_config(config_with_secrets());
        assert_eq!(
            redacted.server.api_token.as_deref(),
            Some(REDACTED_SENTINEL)
        );
        assert_eq!(
            redacted.server.admin_password_hash.as_deref(),
            Some(REDACTED_SENTINEL)
        );
        assert_eq!(redacted.nzbget_api.password, REDACTED_SENTINEL);
        assert_eq!(redacted.feedback.github_token, REDACTED_SENTINEL);
        assert_eq!(
            redacted.webhooks[0].secret.as_deref(),
            Some(REDACTED_SENTINEL)
        );
    }

    #[test]
    fn redact_leaves_unset_fields_unset() {
        let redacted = redact_config(Config::default());
        assert!(redacted.server.api_token.is_none());
        assert!(redacted.server.admin_password_hash.is_none());
        assert_eq!(redacted.nzbget_api.password, "");
        assert_eq!(redacted.feedback.github_token, "");
        assert!(redacted.webhooks.is_empty());
    }

    #[test]
    fn redacted_response_does_not_serialize_real_secrets() {
        // Belt-and-braces: serialise the redacted config to JSON and grep
        // for the plaintext secret values. They must not appear anywhere.
        let cfg = config_with_secrets();
        let redacted = redact_config(cfg);
        let json = serde_json::to_string(&redacted).expect("serialise");
        for needle in [
            "ApiTokenAbc123",
            "$argon2id$v=19$",
            "nzbpass",
            "ghp_realtoken",
            "hmacsecret",
        ] {
            assert!(
                !json.contains(needle),
                "redacted config still contains plaintext '{needle}': {json}"
            );
        }
    }

    #[test]
    fn passthrough_restores_sentinel_secrets_on_put() {
        let existing = config_with_secrets();
        let mut incoming = redact_config(existing.clone());
        // The UI hands the masked sentinel back to us unchanged.
        apply_secret_passthrough(&mut incoming, &existing);
        assert_eq!(incoming.server.api_token, existing.server.api_token);
        assert_eq!(
            incoming.server.admin_password_hash,
            existing.server.admin_password_hash
        );
        assert_eq!(incoming.nzbget_api.password, existing.nzbget_api.password);
        assert_eq!(
            incoming.feedback.github_token,
            existing.feedback.github_token
        );
        assert_eq!(incoming.webhooks[0].secret, existing.webhooks[0].secret);
    }

    #[test]
    fn passthrough_accepts_real_replacement_values() {
        let existing = config_with_secrets();
        let mut incoming = config_with_secrets();
        incoming.feedback.github_token = "ghp_replaced".into();
        incoming.nzbget_api.password = "newpass".into();
        apply_secret_passthrough(&mut incoming, &existing);
        // Non-sentinel secrets must come through unchanged so operators
        // can actually rotate them.
        assert_eq!(incoming.feedback.github_token, "ghp_replaced");
        assert_eq!(incoming.nzbget_api.password, "newpass");
    }
}
