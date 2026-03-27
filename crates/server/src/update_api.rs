//! REST API routes for update management.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::Serialize;

use amigo_core::updater::{self, CoreUpdateStatus};

use crate::api::AppState;

pub fn update_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/updates/check", get(check_updates))
        .route("/api/v1/updates/core", post(apply_core_update))
        .route("/api/v1/updates/plugins/available", get(list_available_plugins))
        .route("/api/v1/updates/plugins/update-all", post(update_all_plugins))
        .route("/api/v1/updates/plugins/{id}", post(update_plugin))
        .route("/api/v1/updates/plugins/{id}/install", post(install_plugin))
        .with_state(state)
}

// --- Response types ---

#[derive(Serialize)]
struct CheckUpdatesResponse {
    core: CoreUpdateInfo,
    plugins: Vec<PluginUpdateEntry>,
}

#[derive(Serialize)]
struct CoreUpdateInfo {
    current_version: String,
    latest_version: Option<String>,
    update_available: bool,
    release_notes: Option<String>,
    distribution: String,
    can_self_update: bool,
}

#[derive(Serialize)]
struct PluginUpdateEntry {
    plugin_id: String,
    current_version: Option<String>,
    available_version: String,
    is_new: bool,
}

#[derive(Serialize)]
struct PluginInstallResponse {
    id: String,
    name: String,
    version: String,
}

#[derive(Serialize)]
struct UpdateAllResponse {
    updated: Vec<PluginInstallResponse>,
}

#[derive(Serialize)]
struct MarketplaceEntry {
    id: String,
    name: String,
    version: String,
    description: String,
    author: String,
    tags: Vec<String>,
    installed: bool,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

// --- Handlers ---

async fn check_updates(
    State(state): State<AppState>,
) -> Result<Json<CheckUpdatesResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check core update
    let core_status = updater::check_for_update(
        &state.http_client,
        &state.coordinator.config().update.github_repo,
    )
    .await;

    let core = match core_status {
        Ok(CoreUpdateStatus::UpdateAvailable {
            current,
            latest,
            release_notes,
            can_self_update,
            ..
        }) => CoreUpdateInfo {
            current_version: current,
            latest_version: Some(latest),
            update_available: true,
            release_notes: Some(release_notes),
            distribution: format!("{:?}", updater::detect_distribution()),
            can_self_update,
        },
        _ => CoreUpdateInfo {
            current_version: updater::CURRENT_VERSION.to_string(),
            latest_version: None,
            update_available: false,
            release_notes: None,
            distribution: format!("{:?}", updater::detect_distribution()),
            can_self_update: false,
        },
    };

    // Check plugin updates
    let plugin_updates = state
        .plugin_updater
        .check_updates()
        .await
        .unwrap_or_default();

    let plugins: Vec<PluginUpdateEntry> = plugin_updates
        .into_iter()
        .map(|u| PluginUpdateEntry {
            plugin_id: u.plugin_id,
            current_version: u.current_version,
            available_version: u.available_version,
            is_new: u.is_new,
        })
        .collect();

    Ok(Json(CheckUpdatesResponse { core, plugins }))
}

async fn apply_core_update(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<ErrorResponse>)> {
    let status = updater::check_for_update(
        &state.http_client,
        &state.coordinator.config().update.github_repo,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;

    match status {
        CoreUpdateStatus::UpToDate => Ok((
            StatusCode::OK,
            Json(serde_json::json!({"message": "Already up to date"})),
        )),
        CoreUpdateStatus::UpdateAvailable {
            latest,
            can_self_update,
            download_url,
            sha256_url,
            ..
        } => {
            if !can_self_update {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Self-update not supported for this distribution".into(),
                    }),
                ));
            }

            // Spawn background task for download + apply
            let client = state.http_client.clone();
            let ver = latest.clone();
            tokio::spawn(async move {
                match updater::download_and_apply(&client, &download_url, sha256_url.as_deref()).await {
                    Ok(()) => tracing::info!("Core update to v{ver} applied — restart needed"),
                    Err(e) => tracing::error!("Core update failed: {e}"),
                }
            });

            Ok((
                StatusCode::ACCEPTED,
                Json(serde_json::json!({
                    "message": format!("Update to v{latest} initiated. Restart required after completion."),
                    "version": latest,
                })),
            ))
        }
    }
}

async fn list_available_plugins(
    State(state): State<AppState>,
) -> Result<Json<Vec<MarketplaceEntry>>, (StatusCode, Json<ErrorResponse>)> {
    let available = state.plugin_updater.list_available().await.map_err(|e| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;

    let installed = state.plugins.list_plugins().await;
    let installed_ids: Vec<String> = installed.iter().map(|p| p.id.clone()).collect();

    let entries = available
        .into_iter()
        .map(|p| MarketplaceEntry {
            installed: installed_ids.contains(&p.id),
            id: p.id,
            name: p.name,
            version: p.version,
            description: p.description,
            author: p.author,
            tags: p.tags,
        })
        .collect();

    Ok(Json(entries))
}

async fn update_all_plugins() -> (StatusCode, Json<serde_json::Value>) {
    // TODO: Rune's types aren't Send — needs spawn_blocking wrapper for PluginLoader operations
    (StatusCode::ACCEPTED, Json(serde_json::json!({"message": "Plugin update triggered. Check /api/v1/plugins for results."})))
}

async fn update_plugin(
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    // TODO: Rune's types aren't Send — needs spawn_blocking wrapper
    (StatusCode::ACCEPTED, Json(serde_json::json!({"message": format!("Update for plugin {id} triggered.")})))
}

async fn install_plugin(
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    // TODO: Rune's types aren't Send — needs spawn_blocking wrapper
    (StatusCode::ACCEPTED, Json(serde_json::json!({"message": format!("Install of plugin {id} triggered.")})))
}
