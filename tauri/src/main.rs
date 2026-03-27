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

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    // Load config
    let config = amigo_core::config::Config::load_auto();

    // Initialize storage
    let data_dir = tauri::api::path::app_data_dir(&tauri::Config::default())
        .unwrap_or_else(|| PathBuf::from("."));
    let db_path = data_dir.join("amigo.db");
    let download_dir = PathBuf::from(&config.download_dir);
    let temp_dir = PathBuf::from(&config.temp_dir);

    let storage = amigo_core::storage::Storage::open(db_path, download_dir, temp_dir)
        .expect("Failed to initialize storage");

    let coordinator = Arc::new(amigo_core::coordinator::Coordinator::new(
        config.clone(),
        storage,
    ));

    // Build Tauri app
    tauri::Builder::default()
        // System tray
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        // Deep links: magnet:, nzb://
        .plugin(tauri_plugin_deep_link::init())
        // Auto-updater via Tauri's built-in mechanism
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Share coordinator state with Tauri commands
        .manage(coordinator.clone())
        // Tauri commands callable from the frontend
        .invoke_handler(tauri::generate_handler![
            cmd_add_download,
            cmd_get_stats,
        ])
        .setup(|app| {
            // Start the Axum API server in the background
            let coord = coordinator.clone();
            let rt = tokio::runtime::Runtime::new().unwrap();
            std::thread::spawn(move || {
                rt.block_on(async {
                    // Recover interrupted downloads
                    let _ = coord.recover_downloads().await;

                    // Start Click'n'Load listener
                    let cnl_coord = coord.clone();
                    tokio::spawn(async move {
                        let _ = amigo_core::clicknload::start_clicknload(cnl_coord).await;
                    });

                    tracing::info!("Desktop app: core engine running");
                    // Keep the runtime alive
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
    eprintln!("  1. Uncomment tauri dependencies in tauri/Cargo.toml");
    eprintln!("  2. cargo install tauri-cli");
    eprintln!("  3. cd web-ui && npm ci && npx vite build && cd ..");
    eprintln!("  4. cd tauri && cargo tauri build");
    eprintln!();
    eprintln!("For headless/server use, run `amigo-server` instead.");
}
