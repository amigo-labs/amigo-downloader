//! Tauri v2 desktop application for amigo-downloader.
//!
//! This embeds the core download engine and serves the web-ui
//! in a native window with system tray integration.
//!
//! ## Building
//!
//! ```bash
//! # Install prerequisites
//! cargo install tauri-cli
//! cd web-ui && npm ci && npx vite build && cd ..
//!
//! # Build desktop app
//! cd tauri && cargo tauri build
//! ```

// NOTE: This file is a complete Tauri v2 app implementation.
// It compiles when the `tauri` dependency is uncommented in Cargo.toml
// and built via `cargo tauri build`. It does NOT compile as a regular
// `cargo build` since Tauri requires its own build toolchain.

#[cfg(feature = "tauri")]
fn main() {
    use std::path::PathBuf;
    use std::sync::Arc;
    use tauri::Manager;

    // Signal to amigo-core that we are running inside Tauri.
    // This must be set before any core initialization so that
    // detect_distribution() returns Distribution::Tauri.
    // SAFETY: Called at the very start of main, before any threads are spawned.
    unsafe {
        std::env::set_var("AMIGO_TAURI", "1");
    }

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    // Load config
    let config = amigo_core::config::Config::load_auto();

    // Build Tauri app — storage is initialized in setup() where we have
    // access to the app handle for resolving platform-specific data dirs.
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            cmd_add_download,
            cmd_get_stats,
        ])
        .setup(move |app| {
            // Resolve platform data dir via Tauri v2 API
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            let db_path = data_dir.join("amigo.db");
            let download_dir = PathBuf::from(&config.download_dir);
            let temp_dir = PathBuf::from(&config.temp_dir);

            let storage = amigo_core::storage::Storage::open(db_path, download_dir, temp_dir)
                .expect("Failed to initialize storage");

            let coordinator = Arc::new(amigo_core::coordinator::Coordinator::new(
                config.clone(),
                storage,
            ));

            // Share coordinator state with Tauri commands
            app.manage(coordinator.clone());

            // Start the core engine in the background
            let coord = coordinator.clone();
            let rt = tokio::runtime::Runtime::new().unwrap();
            std::thread::spawn(move || {
                rt.block_on(async {
                    let _ = coord.recover_downloads().await;

                    // Start Click'n'Load listener on port 9666
                    let cnl_coord = coord.clone();
                    tokio::spawn(async move {
                        if let Err(e) = amigo_server::clicknload::start_clicknload(cnl_coord).await {
                            tracing::warn!("Click'n'Load failed to start: {e}");
                        }
                    });

                    tracing::info!("Desktop app: core engine running");
                    tokio::signal::ctrl_c().await.ok();
                });
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Error running amigo-downloader desktop app");
}

// Tauri commands (callable from JavaScript)
#[cfg(feature = "tauri")]
#[tauri::command]
async fn cmd_add_download(
    coordinator: tauri::State<'_, std::sync::Arc<amigo_core::coordinator::Coordinator>>,
    url: String,
) -> Result<String, String> {
    coordinator
        .add_download(&url, None)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(feature = "tauri")]
#[tauri::command]
async fn cmd_get_stats(
    coordinator: tauri::State<'_, std::sync::Arc<amigo_core::coordinator::Coordinator>>,
) -> Result<serde_json::Value, String> {
    let active = coordinator.active_count().await;
    let speed = coordinator.total_speed().await;
    Ok(serde_json::json!({
        "active_downloads": active,
        "speed_bytes_per_sec": speed,
    }))
}

// When built without Tauri (regular cargo build), show instructions
#[cfg(not(feature = "tauri"))]
fn main() {
    eprintln!("amigo-downloader desktop app");
    eprintln!();
    eprintln!("This binary must be built with the Tauri CLI:");
    eprintln!("  1. cargo install tauri-cli");
    eprintln!("  2. cd web-ui && npm ci && npx vite build && cd ..");
    eprintln!("  3. cd tauri && cargo tauri build");
    eprintln!();
    eprintln!("For headless/server use, run `amigo-server` instead.");
}
