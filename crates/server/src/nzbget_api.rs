//! NZBGet-compatible JSON-RPC API for Sonarr/Radarr integration.
//!
//! Implements the NZBGet JSON-RPC protocol so that Sonarr, Radarr, and other
//! *arr tools can use amigo-downloader as a drop-in NZBGet replacement.
//!
//! Endpoint: POST /jsonrpc (with HTTP Basic Auth)
//! Protocol: JSON-RPC 1.0 (positional parameters only)

use std::sync::Arc;
use std::time::Instant;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::{Json, Router, routing::post};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{debug, info, warn};

use amigo_core::coordinator::Coordinator;
use amigo_core::queue::QueueStatus;

use crate::api::AppState;

/// Server start time for uptime calculation.
static SERVER_START: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);

/// NZBGet-compatible JSON-RPC router.
pub fn nzbget_router(state: AppState) -> Router {
    // Initialize the start time
    let _ = *SERVER_START;

    Router::new()
        .route("/jsonrpc", post(jsonrpc_handler))
        .route("/{_username}/jsonrpc", post(jsonrpc_handler))
        .with_state(state)
}

// --- JSON-RPC types ---

#[derive(Deserialize)]
struct JsonRpcRequest {
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Vec<Value>,
}

#[derive(Serialize)]
#[allow(dead_code)]
struct JsonRpcResponse {
    id: Value,
    result: Value,
}

#[derive(Serialize)]
#[allow(dead_code)]
struct JsonRpcError {
    id: Value,
    error: JsonRpcErrorDetail,
}

#[derive(Serialize)]
#[allow(dead_code)]
struct JsonRpcErrorDetail {
    code: i32,
    message: String,
}

// --- Auth ---

fn check_basic_auth(headers: &HeaderMap, expected_user: &str, expected_pass: &str) -> bool {
    // If both username and password are empty, auth is disabled
    if expected_user.is_empty() && expected_pass.is_empty() {
        return true;
    }

    let Some(auth) = headers.get("authorization") else {
        return false;
    };
    let Ok(auth_str) = auth.to_str() else {
        return false;
    };
    let Some(encoded) = auth_str.strip_prefix("Basic ") else {
        return false;
    };

    let decoded = match base64::engine::general_purpose::STANDARD.decode(encoded.trim()) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => return false,
    };

    // Format: "username:password"
    let Some((user, pass)) = decoded.split_once(':') else {
        return false;
    };

    user == expected_user && pass == expected_pass
}

// --- Handler ---

async fn jsonrpc_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    let config = state.coordinator.config().await;
    if !check_basic_auth(&headers, &config.nzbget_api.username, &config.nzbget_api.password) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"id": null, "error": {"code": -32000, "message": "Unauthorized"}})),
        );
    }

    let id = req.id.unwrap_or(Value::Null);
    let method = req.method.as_str();

    debug!("NZBGet JSON-RPC: {method}");

    let result = match method {
        "version" => handle_version(),
        "status" => handle_status(&state.coordinator).await,
        "append" => handle_append(&state.coordinator, &req.params).await,
        "listgroups" => handle_listgroups(&state.coordinator).await,
        "history" => handle_history(&state.coordinator).await,
        "editqueue" => handle_editqueue(&state.coordinator, &req.params).await,
        "config" => handle_config(&state.coordinator).await,
        "postqueue" => Ok(json!([])),
        "listfiles" => Ok(json!([])),
        "log" => Ok(json!([])),
        "writelog" => Ok(json!(true)),
        "loadlog" => Ok(json!([])),
        "servervolumes" => Ok(json!([])),
        "resetservervolume" => Ok(json!(true)),
        "rate" => Ok(json!(true)),
        "pausedownload" => {
            // Global pause — pause all active downloads
            handle_global_pause(&state.coordinator).await
        }
        "resumedownload" => {
            // Global resume — resume all paused downloads
            handle_global_resume(&state.coordinator).await
        }
        "scan" => Ok(json!(true)),
        "scheduleresume" => Ok(json!(true)),
        _ => {
            warn!("NZBGet JSON-RPC: unknown method '{method}'");
            Err((-32601, format!("Method not found: {method}")))
        }
    };

    match result {
        Ok(value) => (
            StatusCode::OK,
            Json(json!({"id": id, "result": value})),
        ),
        Err((code, msg)) => (
            StatusCode::OK, // JSON-RPC errors still return 200
            Json(json!({"id": id, "error": {"code": code, "message": msg}})),
        ),
    }
}

type RpcResult = Result<Value, (i32, String)>;

// --- Method implementations ---

fn handle_version() -> RpcResult {
    Ok(json!("amigo-dl-0.1.0"))
}

async fn handle_status(coordinator: &Arc<Coordinator>) -> RpcResult {
    let speed = coordinator.total_speed().await;
    let active = coordinator.active_count().await;
    let uptime = SERVER_START.elapsed().as_secs();

    // Calculate remaining size from queued + downloading
    let remaining: u64 = coordinator
        .storage()
        .list_downloads_by_status(QueueStatus::Queued)
        .await
        .unwrap_or_default()
        .iter()
        .chain(
            coordinator
                .storage()
                .list_downloads_by_status(QueueStatus::Downloading)
                .await
                .unwrap_or_default()
                .iter(),
        )
        .map(|d| d.filesize.unwrap_or(0).saturating_sub(d.bytes_downloaded))
        .sum();

    Ok(json!({
        "RemainingSizeLo": (remaining & 0xFFFFFFFF) as u32,
        "RemainingSizeHi": (remaining >> 32) as u32,
        "RemainingSizeMB": remaining / (1024 * 1024),
        "DownloadedSizeLo": 0,
        "DownloadedSizeHi": 0,
        "DownloadedSizeMB": 0,
        "DownloadLimit": coordinator.config().await.bandwidth.global_limit,
        "AverageDownloadRate": speed,
        "DownloadRate": speed,
        "ThreadCount": active,
        "PostJobCount": 0,
        "UrlCount": 0,
        "UpTimeSec": uptime,
        "DownloadTimeSec": uptime,
        "ServerPaused": false,
        "DownloadPaused": false,
        "Download2Paused": false,
        "ServerTime": chrono::Utc::now().timestamp(),
        "ResumeTime": 0,
        "FreeDiskSpaceLo": 0,
        "FreeDiskSpaceHi": 0,
        "FreeDiskSpaceMB": 0,
        "ServerStandBy": active == 0,
        "NewsServers": [],
    }))
}

async fn handle_append(coordinator: &Arc<Coordinator>, params: &[Value]) -> RpcResult {
    // append(NZBFilename, NZBContent, Category, Priority, AddToTop, AddPaused, DupeKey, DupeScore, DupeMode, PPParameters)
    let nzb_filename = params.first().and_then(|v| v.as_str()).unwrap_or("upload.nzb");
    let nzb_content = params.get(1).and_then(|v| v.as_str()).unwrap_or("");
    let _category = params.get(2).and_then(|v| v.as_str()).unwrap_or("");
    let _priority = params.get(3).and_then(|v| v.as_i64()).unwrap_or(0);
    let _add_paused = params.get(5).and_then(|v| v.as_bool()).unwrap_or(false);

    if nzb_content.is_empty() {
        return Err((-32602, "NZBContent is required".into()));
    }

    // NZBContent can be base64-encoded NZB data or a URL
    let nzb_data = if nzb_content.starts_with("http://") || nzb_content.starts_with("https://") {
        // URL mode — we'd need to fetch it, for now just add as URL download
        match coordinator
            .add_download(nzb_content, Some(nzb_filename.to_string()))
            .await
        {
            Ok(id) => {
                info!("NZBGet append (URL): {nzb_filename} → {id}");
                // Return a positive integer as NZBID
                let nzbid = id_to_nzbid(&id);
                return Ok(json!(nzbid));
            }
            Err(e) => return Err((-32000, format!("Failed to add download: {e}"))),
        }
    } else {
        // Base64-encoded NZB content
        base64::engine::general_purpose::STANDARD
            .decode(nzb_content)
            .map_err(|e| (-32602, format!("Invalid base64 NZB content: {e}")))?
    };

    let nzb_str = String::from_utf8(nzb_data)
        .map_err(|e| (-32602, format!("Invalid UTF-8 in NZB: {e}")))?;

    // Validate NZB
    amigo_core::protocol::usenet::nzb::parse_nzb(&nzb_str)
        .map_err(|e| (-32602, format!("Invalid NZB: {e}")))?;

    // Add as usenet download
    match coordinator
        .add_download("nzb://upload", Some(nzb_filename.to_string()))
        .await
    {
        Ok(id) => {
            info!("NZBGet append: {nzb_filename} → {id}");
            let nzbid = id_to_nzbid(&id);
            Ok(json!(nzbid))
        }
        Err(e) => Err((-32000, format!("Failed to add download: {e}"))),
    }
}

async fn handle_listgroups(coordinator: &Arc<Coordinator>) -> RpcResult {
    let rows = coordinator
        .storage()
        .list_downloads()
        .await
        .unwrap_or_default();

    let groups: Vec<Value> = rows
        .iter()
        .filter(|r| r.status != "completed" && r.status != "failed")
        .map(|r| {
            let filesize = r.filesize.unwrap_or(0);
            let downloaded = r.bytes_downloaded;
            let remaining = filesize.saturating_sub(downloaded);
            let nzbid = id_to_nzbid(&r.id);

            let status = match r.status.as_str() {
                "downloading" => "DOWNLOADING",
                "queued" => "QUEUED",
                "paused" => "PAUSED",
                _ => "QUEUED",
            };

            json!({
                "NZBID": nzbid,
                "NZBName": r.filename.as_deref().unwrap_or("download"),
                "NZBFilename": r.filename.as_deref().unwrap_or("download.nzb"),
                "NZBNicename": r.filename.as_deref().unwrap_or("download"),
                "Kind": "NZB",
                "URL": r.url,
                "DestDir": r.download_dir.as_deref().unwrap_or("downloads"),
                "FinalDir": "",
                "Category": "",
                "FileSizeLo": (filesize & 0xFFFFFFFF) as u32,
                "FileSizeHi": (filesize >> 32) as u32,
                "FileSizeMB": filesize / (1024 * 1024),
                "DownloadedSizeLo": (downloaded & 0xFFFFFFFF) as u32,
                "DownloadedSizeHi": (downloaded >> 32) as u32,
                "DownloadedSizeMB": downloaded / (1024 * 1024),
                "RemainingSizeLo": (remaining & 0xFFFFFFFF) as u32,
                "RemainingSizeHi": (remaining >> 32) as u32,
                "RemainingSizeMB": remaining / (1024 * 1024),
                "ActiveDownloads": if r.status == "downloading" { 1 } else { 0 },
                "Status": status,
                "TotalArticles": 0,
                "SuccessArticles": 0,
                "FailedArticles": 0,
                "Health": 1000,
                "CriticalHealth": 1000,
                "MaxPriority": r.priority,
                "MinPriority": r.priority,
                "MinPostTime": 0,
                "MaxPostTime": 0,
                "Parameters": [],
                "ScriptStatuses": [],
                "ServerStats": [],
                "PostInfoText": "",
                "PostStageProgress": 0,
                "PostTotalTimeSec": 0,
                "PostStageTimeSec": 0,
            })
        })
        .collect();

    Ok(json!(groups))
}

async fn handle_history(coordinator: &Arc<Coordinator>) -> RpcResult {
    // Get completed + failed downloads
    let completed = coordinator
        .storage()
        .list_downloads_by_status(QueueStatus::Completed)
        .await
        .unwrap_or_default();
    let failed = coordinator
        .storage()
        .list_downloads_by_status(QueueStatus::Failed)
        .await
        .unwrap_or_default();

    // Also include actual history
    let history_rows = coordinator
        .storage()
        .get_history()
        .await
        .unwrap_or_default();

    let items: Vec<Value> = completed
        .iter()
        .chain(failed.iter())
        .chain(history_rows.iter())
        .map(|r| {
            let filesize = r.filesize.unwrap_or(0);
            let downloaded = r.bytes_downloaded;
            let nzbid = id_to_nzbid(&r.id);
            let is_success = r.status == "completed";

            json!({
                "NZBID": nzbid,
                "NZBName": r.filename.as_deref().unwrap_or("download"),
                "NZBFilename": r.filename.as_deref().unwrap_or("download.nzb"),
                "NZBNicename": r.filename.as_deref().unwrap_or("download"),
                "Kind": "NZB",
                "URL": r.url,
                "DestDir": r.download_dir.as_deref().unwrap_or("downloads"),
                "FinalDir": r.download_dir.as_deref().unwrap_or("downloads"),
                "Category": "",
                "FileSizeLo": (filesize & 0xFFFFFFFF) as u32,
                "FileSizeHi": (filesize >> 32) as u32,
                "FileSizeMB": filesize / (1024 * 1024),
                "DownloadedSizeLo": (downloaded & 0xFFFFFFFF) as u32,
                "DownloadedSizeHi": (downloaded >> 32) as u32,
                "DownloadedSizeMB": downloaded / (1024 * 1024),
                "Status": if is_success { "SUCCESS/ALL" } else { "FAILURE/NONE" },
                "ParStatus": if is_success { "SUCCESS" } else { "NONE" },
                "ExParStatus": "RECIPIENT",
                "UnpackStatus": if is_success { "SUCCESS" } else { "NONE" },
                "MoveStatus": if is_success { "SUCCESS" } else { "NONE" },
                "ScriptStatus": "NONE",
                "DeleteStatus": "NONE",
                "MarkStatus": "NONE",
                "UrlStatus": "NONE",
                "TotalArticles": 0,
                "SuccessArticles": 0,
                "FailedArticles": 0,
                "Health": 1000,
                "CriticalHealth": 1000,
                "DupeKey": "",
                "DupeScore": 0,
                "DupeMode": "SCORE",
                "Deleted": false,
                "DownloadTimeSec": 0,
                "PostTotalTimeSec": 0,
                "ParTimeSec": 0,
                "RepairTimeSec": 0,
                "UnpackTimeSec": 0,
                "Parameters": [],
                "ScriptStatuses": [],
                "ServerStats": [],
            })
        })
        .collect();

    Ok(json!(items))
}

async fn handle_editqueue(coordinator: &Arc<Coordinator>, params: &[Value]) -> RpcResult {
    let command = params.first().and_then(|v| v.as_str()).unwrap_or("");
    let _param = params.get(1).and_then(|v| v.as_str()).unwrap_or("");
    let ids = params
        .get(2)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    for id_val in &ids {
        let nzbid = id_val.as_i64().unwrap_or(0);
        if nzbid <= 0 {
            continue;
        }

        // Find download by nzbid
        let all = coordinator.storage().list_downloads().await.unwrap_or_default();
        let download = all.iter().find(|d| id_to_nzbid(&d.id) == nzbid as i32);

        if let Some(dl) = download {
            let result = match command {
                "GroupPause" | "GroupPauseAllPars" | "GroupPauseExtraPars" => {
                    coordinator.pause(&dl.id).await
                }
                "GroupResume" => coordinator.resume(&dl.id).await,
                "GroupDelete" | "GroupFinalDelete" => coordinator.cancel(&dl.id).await,
                "HistoryDelete" | "HistoryFinalDelete" => coordinator.cancel(&dl.id).await,
                _ => {
                    debug!("NZBGet editqueue: unsupported command '{command}'");
                    Ok(())
                }
            };

            if let Err(e) = result {
                warn!("NZBGet editqueue {command} failed for {}: {e}", dl.id);
            }
        }
    }

    Ok(json!(true))
}

async fn handle_config(coordinator: &Arc<Coordinator>) -> RpcResult {
    let config = coordinator.config().await;
    Ok(json!([
        {"Name": "MainDir", "Value": config.download_dir},
        {"Name": "DestDir", "Value": config.download_dir},
        {"Name": "TempDir", "Value": config.temp_dir},
        {"Name": "NzbDir", "Value": ""},
        {"Name": "ServerPort", "Value": "1516"},
        {"Name": "ControlUsername", "Value": ""},
        {"Name": "ControlPassword", "Value": ""},
        {"Name": "Category1.Name", "Value": ""},
        {"Name": "Category1.DestDir", "Value": ""},
        {"Name": "ParCheck", "Value": if config.usenet.par2_repair { "auto" } else { "no" }},
        {"Name": "Unpack", "Value": if config.usenet.auto_unrar { "yes" } else { "no" }},
    ]))
}

async fn handle_global_pause(coordinator: &Arc<Coordinator>) -> RpcResult {
    let downloading = coordinator
        .storage()
        .list_downloads_by_status(QueueStatus::Downloading)
        .await
        .unwrap_or_default();

    for dl in &downloading {
        let _ = coordinator.pause(&dl.id).await;
    }

    Ok(json!(true))
}

async fn handle_global_resume(coordinator: &Arc<Coordinator>) -> RpcResult {
    let paused = coordinator
        .storage()
        .list_downloads_by_status(QueueStatus::Paused)
        .await
        .unwrap_or_default();

    for dl in &paused {
        let _ = coordinator.resume(&dl.id).await;
    }

    Ok(json!(true))
}

// --- Helpers ---

/// Convert a UUID string to a stable integer NZBID.
/// NZBGet uses positive integers as IDs. We hash the UUID to get a stable int.
fn id_to_nzbid(id: &str) -> i32 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    // Ensure positive, non-zero
    (hasher.finish() as i32).abs().max(1)
}
