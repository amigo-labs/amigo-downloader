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

    let app = api::router(coordinator.clone())
        .merge(ws::ws_router(coordinator.clone()))
        .layer(CorsLayer::permissive());

    let bind = "0.0.0.0:8080";
    tracing::info!("Starting amigo-server on {bind}");

    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
