//! amigo-server library — exposes API router and types for integration tests.

pub mod api;
mod background;
pub mod clicknload;
mod feedback;
mod nzbget_api;
mod resolver;
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
