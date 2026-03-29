mod api;
mod clicknload;
mod feedback;
mod static_files;
mod update_api;
pub mod webhooks;
mod ws;

use std::path::PathBuf;
use std::sync::Arc;

use tower_http::cors::CorsLayer;
use tracing_subscriber::EnvFilter;

use amigo_core::captcha::CaptchaManager;
use amigo_core::config::Config;
use amigo_core::coordinator::{Coordinator, DownloadEvent};
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

    // Create captcha manager
    let captcha_manager = Arc::new(CaptchaManager::new(
        coordinator.event_sender(),
        &config.captcha,
    ));

    // Load plugins with notify + captcha callbacks wired
    let mut plugin_host_api = amigo_plugin_runtime::host_api::HostApi::new(
        SandboxLimits::default().max_http_requests,
    );

    // Wire notify callback: plugin → broadcast → WebSocket → UI toast
    {
        let event_tx = coordinator.event_sender();
        plugin_host_api.set_notify_callback(Arc::new(move |plugin_id, title, message| {
            let _ = event_tx.send(DownloadEvent::PluginNotification {
                plugin_id: plugin_id.to_string(),
                title: title.to_string(),
                message: message.to_string(),
            });
        }));
    }

    // Wire captcha callback: plugin → CaptchaManager → WebSocket → UI → REST → plugin
    {
        let cm = captcha_manager.clone();
        plugin_host_api.set_captcha_callback(Arc::new(move |plugin_id, download_id, image_url, captcha_type| {
            let cm = cm.clone();
            Box::pin(async move {
                cm.request_solve(&plugin_id, &download_id, &image_url, &captcha_type)
                    .await
                    .map_err(|e| e.to_string())
            })
        }));
    }

    let plugin_loader = Arc::new(
        PluginLoader::new_with_host_api(
            PathBuf::from("plugins"),
            SandboxLimits::default(),
            plugin_host_api,
        )
        .expect("Failed to initialize plugin runtime — cannot start server"),
    );
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

    // Webhook dispatcher
    let webhook_dispatcher = Arc::new(webhooks::WebhookDispatcher::new(config.webhooks.clone()));
    {
        let dispatcher = webhook_dispatcher.clone();
        let event_rx = coordinator.subscribe();
        tokio::spawn(async move {
            dispatcher.run(event_rx).await;
        });
    }

    let state = api::AppState {
        coordinator: coordinator.clone(),
        plugins: plugin_loader,
        plugin_updater,
        http_client,
        captcha_manager,
        webhook_dispatcher,
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

    let bind = "0.0.0.0:1516";
    tracing::info!("Starting amigo-server on {bind}");

    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
