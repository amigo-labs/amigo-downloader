//! amigo-server library — exposes API router and types for integration tests.
#![allow(dead_code)]

pub mod api;
pub mod auth;
mod background;
pub mod clicknload;
mod feedback;
pub mod login;
pub mod net_guard;
mod nzbget_api;
pub mod pairing;
pub mod password;
mod resolver;
pub mod setup;
mod static_files;
mod update_api;
pub mod webhooks;
pub mod ws;

use std::path::PathBuf;
use std::sync::Arc;

use amigo_core::captcha::CaptchaManager;
use amigo_core::config::Config;
use amigo_core::coordinator::Coordinator;
use amigo_core::storage::Storage;
use amigo_plugin_runtime::loader::PluginLoader;
use amigo_plugin_runtime::sandbox::SandboxLimits;
use amigo_plugin_runtime::updater::PluginUpdater;

/// Build the full application state suitable for testing.
/// Uses in-memory storage and a no-discover plugin loader.
pub fn build_test_state(config: Config) -> api::AppState {
    let storage = Storage::open_memory().expect("Failed to open in-memory storage");
    let coordinator = Coordinator::new(config.clone(), storage);
    let coordinator = Arc::new(coordinator);

    let http_client = reqwest::Client::builder()
        .user_agent("amigo-test")
        .build()
        .expect("Failed to build HTTP client");

    let captcha_manager = Arc::new(CaptchaManager::new(
        coordinator.event_sender(),
        &config.captcha,
    ));

    let plugin_loader = Arc::new(
        PluginLoader::new(PathBuf::from("plugins"), SandboxLimits::default())
            .expect("Failed to init plugin loader"),
    );

    let registry_config = amigo_plugin_runtime::registry::RegistryConfig::default();
    let plugin_updater = Arc::new(PluginUpdater::new(
        registry_config,
        http_client.clone(),
        plugin_loader.clone(),
    ));

    let webhook_dispatcher = Arc::new(webhooks::WebhookDispatcher::new(Vec::new()));

    let config_path = std::env::temp_dir().join(format!(
        "amigo-test-config-{}.toml",
        uuid::Uuid::new_v4()
    ));

    api::AppState {
        coordinator,
        plugins: plugin_loader,
        plugin_updater,
        http_client,
        captcha_manager,
        webhook_dispatcher,
        config_path,
    }
}

/// Build the full Axum router for testing (API + WS, no static files).
pub fn build_test_router(state: api::AppState) -> axum::Router {
    api::router(state.clone())
        .merge(ws::ws_router(state.clone()))
}

/// Build the full router including setup / login / pairing routes and the
/// auth + setup-guard middleware, matching production wiring. Used by the
/// end-to-end integration tests.
pub async fn build_full_test_router(
    state: api::AppState,
    setup_pin: Option<String>,
    bind_is_loopback: bool,
) -> axum::Router {
    let config = state.coordinator.config().await;
    let auth_state = auth::AuthState::new(
        state.clone(),
        config.server.api_token.clone(),
        setup_pin,
        config.server.setup_complete,
        bind_is_loopback,
    );
    let auth_layer = axum::middleware::from_fn_with_state(auth_state.clone(), auth::require_auth);
    let setup_guard_layer =
        axum::middleware::from_fn_with_state(auth_state.clone(), auth::setup_guard);

    let protected = api::router(state.clone())
        .merge(ws::ws_router(state.clone()))
        .layer(auth_layer);

    let open = setup::setup_router(state.clone(), auth_state.clone())
        .merge(login::login_router(state.clone(), auth_state.clone()))
        .merge(pairing::pairing_router(state.clone(), auth_state.clone()));

    protected.merge(open).layer(setup_guard_layer)
}
