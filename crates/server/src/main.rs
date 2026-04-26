mod api;
mod auth;
mod background;
mod clicknload;
mod feedback;
mod login;
mod net_guard;
mod nzbget_api;
mod pairing;
mod password;
mod resolver;
mod setup;
mod static_files;
mod update_api;
pub mod webhooks;
mod ws;

use std::path::PathBuf;
use std::sync::Arc;

use axum::http::{HeaderValue, Method, header};
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

    let config_path = Config::resolve_path();
    let mut config = Config::load(&config_path).unwrap_or_default();

    // Env overrides — useful for Docker / IaC where flipping `bind` or
    // enabling reverse-proxy awareness shouldn't require editing a TOML
    // file that also lives in a volume.
    if let Ok(bind) = std::env::var("AMIGO_BIND")
        && !bind.trim().is_empty()
    {
        config.server.bind = bind;
    }
    if std::env::var("AMIGO_TRUST_PROXY")
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
    {
        config.server.trust_proxy = true;
    }
    if std::env::var("AMIGO_AUTO_UPDATE_PLUGINS")
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
    {
        config.update.auto_update_plugins = true;
    }

    // Reject misconfigured bind/token combinations early.
    let validation_errors = config.validate();
    if !validation_errors.is_empty() {
        for err in &validation_errors {
            tracing::error!("Config error: {err}");
        }
        anyhow::bail!("Invalid configuration — refusing to start");
    }

    // Headless setup: if `AMIGO_SETUP_USER` + `AMIGO_SETUP_PASSWORD` are
    // provided and the wizard has not been completed, finish it now so the
    // server comes up fully authenticated without the browser.
    if !config.server.setup_complete
        && let (Ok(user), Ok(pw)) = (
            std::env::var("AMIGO_SETUP_USER"),
            std::env::var("AMIGO_SETUP_PASSWORD"),
        )
    {
        if user.trim().is_empty() || pw.len() < 8 {
            tracing::error!(
                "AMIGO_SETUP_USER / AMIGO_SETUP_PASSWORD provided but invalid (user empty or password < 8 chars)"
            );
            anyhow::bail!("invalid setup env vars");
        }
        let hash = password::hash_password(&pw)
            .map_err(|e| anyhow::anyhow!("setup hash failed: {e}"))?;
        config.server.admin_username = Some(user);
        config.server.admin_password_hash = Some(hash);
        config.server.setup_complete = true;
        config.save(&config_path)?;
        tracing::info!("Setup completed from environment variables — admin account active.");
    }

    let storage = Storage::open(
        PathBuf::from("amigo.db"),
        PathBuf::from(&config.download_dir),
        PathBuf::from(&config.temp_dir),
    )?;

    let mut coordinator = Coordinator::new(config.clone(), storage);

    // Shared HTTP client
    let http_client = reqwest::Client::builder()
        .user_agent("amigo-downloader")
        .build()?;

    // Create captcha manager
    let captcha_manager = Arc::new(CaptchaManager::new(
        coordinator.event_sender(),
        &config.captcha,
    ));

    // Load plugins with notify + captcha callbacks wired. `from_sandbox`
    // picks up the SSRF policy and request limit from the same struct that
    // governs other sandbox limits.
    let sandbox_limits = SandboxLimits::default();
    let mut plugin_host_api =
        amigo_plugin_runtime::host_api::HostApi::from_sandbox(&sandbox_limits);

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
            sandbox_limits,
            plugin_host_api,
        )
        .expect("Failed to initialize plugin runtime — cannot start server"),
    );
    let discovered = plugin_loader.discover().await.unwrap_or_default();
    tracing::info!("Loaded {} plugins", discovered.len());

    // Wire URL resolvers: plugins handle all URL resolution (YouTube, generic-http, etc.)
    let plugin_resolver = resolver::PluginUrlResolver::new(plugin_loader.clone());
    coordinator.set_resolvers(vec![std::sync::Arc::new(plugin_resolver)]);

    let coordinator = std::sync::Arc::new(coordinator);

    // Spawn queue advance loop — starts next queued download when one completes
    coordinator.spawn_queue_advance_loop();

    // Recover any downloads that were in-progress when the server last stopped
    let recovered = coordinator.recover_downloads().await.unwrap_or(0);
    if recovered > 0 {
        tracing::info!("Recovered {recovered} interrupted downloads");
    }

    // Plugin updater. Registry signatures are required by default — a
    // developer bootstrap override (`AMIGO_PLUGIN_REGISTRY_DEV_UNSIGNED=1`)
    // disables verification with a loud warning, for working against a
    // locally-hosted unsigned fork.
    let dev_unsigned = std::env::var("AMIGO_PLUGIN_REGISTRY_DEV_UNSIGNED")
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false);
    if dev_unsigned {
        tracing::warn!(
            "AMIGO_PLUGIN_REGISTRY_DEV_UNSIGNED is set — plugin registry signatures will NOT be verified. DO NOT use this for production."
        );
    }
    let registry_config = RegistryConfig {
        index_url: config.update.plugin_registry_url.clone(),
        trusted_signing_key: if dev_unsigned {
            None
        } else {
            Some(amigo_plugin_runtime::registry::AMIGO_REGISTRY_PUBLIC_KEY)
        },
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
        config_path,
    };

    // Feedback rate limiter
    let feedback_limiter = feedback::new_rate_limiter(config.feedback.max_issues_per_hour);

    // Build the CORS layer from explicit config. Empty = no CORS (same-origin
    // only, which is what the bundled Web UI needs). Permissive was a blanket
    // CSRF hole.
    let cors = if config.server.cors_origins.is_empty() {
        CorsLayer::new()
    } else {
        let origins: Vec<HeaderValue> = config
            .server
            .cors_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
            ])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
    };

    let bind_is_loopback = config.server.is_bind_loopback();
    let setup_pin_env = std::env::var("AMIGO_SETUP_PIN").ok();
    let auth_state = auth::AuthState::new(
        state.clone(),
        config.server.api_token.clone(),
        setup_pin_env,
        config.server.setup_complete,
        bind_is_loopback,
    );
    if !config.server.setup_complete && !bind_is_loopback {
        tracing::warn!(
            "Setup not complete — server is running in setup-mode. Open http://{bind} in a browser to configure the admin account.",
            bind = config.server.bind
        );
    }
    let auth_layer = axum::middleware::from_fn_with_state(
        auth_state.clone(),
        auth::require_auth,
    );
    let setup_guard_layer = axum::middleware::from_fn_with_state(
        auth_state.clone(),
        auth::setup_guard,
    );

    let protected = api::router(state.clone())
        .merge(ws::ws_router(state.clone()))
        .merge(update_api::update_router(state.clone()))
        .merge(feedback::feedback_router(state.clone(), feedback_limiter))
        .layer(auth_layer);

    // Public / partially-authenticated routes (login, /me, setup wizard,
    // pairing endpoints).
    let open = setup::setup_router(state.clone(), auth_state.clone())
        .merge(login::login_router(state.clone(), auth_state.clone()))
        .merge(pairing::pairing_router(state.clone(), auth_state.clone()));

    // NZBGet JSON-RPC has its own HTTP Basic Auth layer (Sonarr/Radarr
    // compatibility) and static files are public. Refuse to start when the
    // operator turned the API on without setting credentials — fail-loud at
    // startup is safer than mounting an open RPC endpoint.
    nzbget_api::validate_nzbget_config(&config.nzbget_api).map_err(|e| anyhow::anyhow!(e))?;
    let mut app = protected.merge(open);
    if let Some(nzb) = nzbget_api::nzbget_router(state.clone(), &config.nzbget_api) {
        app = app.merge(nzb);
    }
    let app = app
        .merge(static_files::static_router())
        .layer(setup_guard_layer)
        .layer(cors);

    // Start background tasks (NZB watch folder, RSS poller)
    background::spawn_background_tasks(
        coordinator.clone(),
        state.http_client.clone(),
        state.plugin_updater.clone(),
    );

    // Start Click'n'Load listener on port 9666 in background
    let cnl_coordinator = coordinator.clone();
    tokio::spawn(async move {
        if let Err(e) = clicknload::start_clicknload(cnl_coordinator).await {
            tracing::warn!("Click'n'Load listener failed: {e}");
        }
    });

    let bind = config.server.bind.clone();
    tracing::info!("Starting amigo-server on {bind}");

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}
