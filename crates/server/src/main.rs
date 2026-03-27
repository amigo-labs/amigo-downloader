mod api;
mod static_files;
mod ws;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let app = api::router();

    let bind = "0.0.0.0:8080";
    tracing::info!("Starting amigo-server on {bind}");

    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
