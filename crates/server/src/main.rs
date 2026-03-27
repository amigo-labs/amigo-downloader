mod api;
mod static_files;
mod ws;

use std::path::PathBuf;
use std::sync::Arc;

use tower_http::cors::CorsLayer;
use tracing_subscriber::EnvFilter;

use amigo_core::config::Config;
use amigo_core::coordinator::Coordinator;
use amigo_core::storage::Storage;
use amigo_plugin_runtime::loader::PluginLoader;
use amigo_plugin_runtime::sandbox::SandboxLimits;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::default();

    let storage = Storage::open(
        PathBuf::from("amigo.db"),
        PathBuf::from(&config.download_dir),
        PathBuf::from(&config.temp_dir),
    )?;

    let coordinator = Arc::new(Coordinator::new(config, storage));

    // Load plugins
    let plugin_loader = Arc::new(PluginLoader::new(
        PathBuf::from("plugins"),
        SandboxLimits::default(),
    ));
    let discovered = plugin_loader.discover().await.unwrap_or_default();
    tracing::info!("Loaded {} plugins", discovered.len());

    let state = api::AppState {
        coordinator: coordinator.clone(),
        plugins: plugin_loader,
    };

    let app = api::router(state.clone())
        .merge(ws::ws_router(state))
        .layer(CorsLayer::permissive());

    let bind = "0.0.0.0:8080";
    tracing::info!("Starting amigo-server on {bind}");

    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
