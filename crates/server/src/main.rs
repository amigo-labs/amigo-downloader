mod api;
mod clicknload;
mod feedback;
mod static_files;
mod update_api;
mod ws;

use std::path::PathBuf;
use std::sync::Arc;

use tower_http::cors::CorsLayer;
use tracing_subscriber::EnvFilter;

use amigo_core::config::Config;
use amigo_core::coordinator::Coordinator;
use amigo_core::storage::Storage;
use amigo_plugin_runtime::loader::PluginLoader;
use amigo_plugin_runtime::registry::RegistryConfig;
use amigo_plugin_runtime::sandbox::SandboxLimits;
use amigo_plugin_runtime::updater::PluginUpdater;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::load_auto();

    let storage = Storage::open(
        PathBuf::from("amigo.db"),
        PathBuf::from(&config.download_dir),
        PathBuf::from(&config.temp_dir),
    )?;

    let coordinator = Arc::new(Coordinator::new(config.clone(), storage));

    // Shared HTTP client
    let http_client = reqwest::Client::builder()
        .user_agent("amigo-downloader")
        .build()?;

    // Load plugins
    let plugin_loader = Arc::new(PluginLoader::new(
        PathBuf::from("plugins"),
        SandboxLimits::default(),
    ));
    let discovered = plugin_loader.discover().await.unwrap_or_default();
    tracing::info!("Loaded {} plugins", discovered.len());

    // Recover any downloads that were in-progress when the server last stopped
    let recovered = coordinator.recover_downloads().await.unwrap_or(0);
    if recovered > 0 {
        tracing::info!("Recovered {recovered} interrupted downloads");
    }

    // Plugin updater
    let registry_config = RegistryConfig {
        index_url: config.update.plugin_registry_url.clone(),
        ..Default::default()
    };
    let plugin_updater = Arc::new(PluginUpdater::new(
        registry_config,
        http_client.clone(),
        plugin_loader.clone(),
    ));

    let state = api::AppState {
        coordinator: coordinator.clone(),
        plugins: plugin_loader,
        plugin_updater,
        http_client,
    };

    // Feedback rate limiter
    let feedback_limiter = feedback::new_rate_limiter(config.feedback.max_issues_per_hour);

    let app = api::router(state.clone())
        .merge(ws::ws_router(state.clone()))
        .merge(update_api::update_router(state.clone()))
        .merge(feedback::feedback_router(state, feedback_limiter))
        .merge(static_files::static_router())
        .layer(CorsLayer::permissive());

    // Start Click'n'Load listener on port 9666 in background
    let cnl_coordinator = coordinator.clone();
    tokio::spawn(async move {
        if let Err(e) = clicknload::start_clicknload(cnl_coordinator).await {
            tracing::warn!("Click'n'Load listener failed: {e}");
        }
    });

    let bind = "0.0.0.0:8080";
    tracing::info!("Starting amigo-server on {bind}");

    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
