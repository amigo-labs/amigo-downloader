use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};

use amigo_core::config::Config;
use amigo_core::coordinator::Coordinator;
use amigo_core::queue::QueueStatus;
use amigo_core::storage::Storage;

#[derive(Parser)]
#[command(name = "amigo", about = "amigo-downloader — fast, modular download manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a download (URL, NZB, or DLC)
    Add {
        /// URL to download
        url: Option<String>,
        /// Import NZB file
        #[arg(long)]
        nzb: Option<String>,
        /// Import DLC container
        #[arg(long)]
        dlc: Option<String>,
    },
    /// Export downloads as DLC container
    ExportDlc {
        /// Comma-separated download IDs (all if omitted)
        #[arg(long)]
        ids: Option<String>,
    },
    /// List active downloads
    List,
    /// Pause a download
    Pause { id: String },
    /// Resume a download
    Resume { id: String },
    /// Cancel a download
    Cancel { id: String },
    /// Show download queue
    Queue,
    /// Show status overview
    Status,
    /// Show current speed
    Speed,
    /// Get or set configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Manage plugins
    Plugins {
        #[command(subcommand)]
        action: PluginAction,
    },
    /// Check for and apply updates
    Update {
        #[command(subcommand)]
        action: UpdateAction,
    },
    /// Start the web server
    Serve {
        #[arg(long, default_value = "8080")]
        port: u16,
        #[arg(long, default_value = "0.0.0.0")]
        bind: String,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    Get { key: String },
    Set { key: String, value: String },
}

#[derive(Subcommand)]
enum PluginAction {
    /// List installed plugins
    List,
    /// Enable a plugin
    Enable { id: String },
    /// Login to a plugin account
    Login { id: String },
    /// Update plugins (all or specific)
    Update { id: Option<String> },
    /// Install a plugin from the registry
    Install { id: String },
    /// Search the plugin registry
    Search { query: String },
}

#[derive(Subcommand)]
enum UpdateAction {
    /// Check for available updates (core + plugins)
    Check,
    /// Apply core binary update
    Apply {
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },
}

fn init_coordinator() -> anyhow::Result<Arc<Coordinator>> {
    let config = Config::load_auto();
    let storage = Storage::open(
        PathBuf::from("amigo.db"),
        PathBuf::from(&config.download_dir),
        PathBuf::from(&config.temp_dir),
    )?;
    Ok(Arc::new(Coordinator::new(config, storage)))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Add { url, nzb, dlc } => {
            let coord = init_coordinator()?;
            if let Some(url) = url {
                let id = coord.add_download(&url, None).await?;
                println!("Added download: {id}");
            } else if let Some(nzb_path) = nzb {
                let data = std::fs::read_to_string(&nzb_path)?;
                let nzb = amigo_core::protocol::usenet::nzb::parse_nzb(&data)?;
                println!("NZB contains {} files", nzb.files.len());
                for file in &nzb.files {
                    println!("  {} ({} segments, {} bytes)", file.filename(), file.segments.len(), file.total_bytes());
                }
                // TODO: start actual Usenet download
                println!("NZB import queued.");
            } else if let Some(dlc_path) = dlc {
                let data = std::fs::read(&dlc_path)?;
                let packages = amigo_core::container::import_dlc(&data)?;
                for pkg in &packages {
                    println!("Package: {} ({} links)", pkg.name, pkg.links.len());
                    for link in &pkg.links {
                        let id = coord.add_download(&link.url, link.filename.clone()).await?;
                        println!("  Added: {} → {id}", link.url);
                    }
                }
            } else {
                println!("Provide a URL, --nzb, or --dlc");
            }
        }

        Commands::ExportDlc { ids } => {
            let coord = init_coordinator()?;
            let downloads = coord.storage().list_downloads().await?;

            let links: Vec<_> = downloads
                .iter()
                .filter(|d| {
                    ids.as_ref()
                        .map(|ids_str| ids_str.split(',').any(|i| i.trim() == d.id))
                        .unwrap_or(true)
                })
                .map(|d| amigo_core::container::ContainerLink {
                    url: d.url.clone(),
                    filename: d.filename.clone(),
                    filesize: d.filesize,
                })
                .collect();

            let pkg = amigo_core::container::ContainerPackage {
                name: "Export".into(),
                links,
            };
            let dlc_data = amigo_core::container::export_dlc(&[pkg])?;
            let out_path = "export.dlc";
            std::fs::write(out_path, &dlc_data)?;
            println!("Exported to {out_path}");
        }

        Commands::List => {
            let coord = init_coordinator()?;
            let downloads = coord.storage().list_downloads().await?;
            if downloads.is_empty() {
                println!("No active downloads.");
            } else {
                for d in &downloads {
                    let pct = d.filesize.map(|s| if s > 0 { d.bytes_downloaded * 100 / s } else { 0 });
                    println!(
                        "[{}] {} — {} {}",
                        d.status,
                        d.filename.as_deref().unwrap_or(&d.url),
                        d.id,
                        pct.map(|p| format!("{p}%")).unwrap_or_default()
                    );
                }
            }
        }

        Commands::Pause { id } => {
            let coord = init_coordinator()?;
            coord.pause(&id).await?;
            println!("Paused: {id}");
        }

        Commands::Resume { id } => {
            let coord = init_coordinator()?;
            coord.resume(&id).await?;
            println!("Resumed: {id}");
        }

        Commands::Cancel { id } => {
            let coord = init_coordinator()?;
            coord.cancel(&id).await?;
            println!("Cancelled: {id}");
        }

        Commands::Queue => {
            let coord = init_coordinator()?;
            let queued = coord.storage().list_downloads_by_status(QueueStatus::Queued).await?;
            if queued.is_empty() {
                println!("Queue is empty.");
            } else {
                for (i, d) in queued.iter().enumerate() {
                    println!("{}. {} — {}", i + 1, d.filename.as_deref().unwrap_or(&d.url), d.id);
                }
            }
        }

        Commands::Status => {
            let coord = init_coordinator()?;
            let active = coord.active_count().await;
            let speed = coord.total_speed().await;
            let queued = coord.storage().count_by_status(QueueStatus::Queued).await?;
            let completed = coord.storage().count_by_status(QueueStatus::Completed).await?;
            let failed = coord.storage().count_by_status(QueueStatus::Failed).await?;

            println!("amigo-downloader v{}", amigo_core::updater::CURRENT_VERSION);
            println!("Active: {active}  Queued: {queued}  Completed: {completed}  Failed: {failed}");
            println!("Speed: {} KB/s", speed / 1024);
        }

        Commands::Speed => {
            let coord = init_coordinator()?;
            let speed = coord.total_speed().await;
            println!("{} KB/s", speed / 1024);
        }

        Commands::Config { action } => {
            let config = Config::load_auto();
            match action {
                ConfigAction::Get { key } => {
                    let json = serde_json::to_value(&config)?;
                    match json.pointer(&format!("/{}", key.replace('.', "/"))) {
                        Some(val) => println!("{key} = {val}"),
                        None => println!("Key not found: {key}"),
                    }
                }
                ConfigAction::Set { key, value } => {
                    println!("Setting {key} = {value}");
                    println!("Config set via CLI not yet fully implemented. Edit config.toml directly.");
                }
            }
        }

        Commands::Plugins { action } => match action {
            PluginAction::List | PluginAction::Enable { .. } | PluginAction::Login { .. } => {
                println!("Plugin management requires the server. Use: amigo serve");
            }
            PluginAction::Update { .. } | PluginAction::Install { .. } | PluginAction::Search { .. } => {
                println!("Plugin updates require the server. Use the web UI or API.");
            }
        },

        Commands::Update { action } => match action {
            UpdateAction::Check => {
                let client = reqwest::Client::builder()
                    .user_agent("amigo-downloader")
                    .build()?;
                let config = Config::load_auto();

                println!("Checking for updates...");

                match amigo_core::updater::check_for_update(&client, &config.update.github_repo).await {
                    Ok(amigo_core::updater::CoreUpdateStatus::UpdateAvailable { current, latest, release_notes, .. }) => {
                        println!("Update available: {current} → {latest}");
                        if !release_notes.is_empty() {
                            println!("\nRelease notes:\n{release_notes}");
                        }
                    }
                    Ok(amigo_core::updater::CoreUpdateStatus::UpToDate) => {
                        println!("Up to date (v{})", amigo_core::updater::CURRENT_VERSION);
                    }
                    Err(e) => {
                        println!("Could not check for updates: {e}");
                    }
                }
            }
            UpdateAction::Apply { yes } => {
                if !yes {
                    println!("Use --yes to confirm the update.");
                    return Ok(());
                }
                println!("Self-update not yet fully implemented.");
            }
        },

        Commands::Serve { port, bind } => {
            println!("Starting server on {bind}:{port}...");
            println!("Use `amigo-server` binary for the full server. This is a placeholder.");
        }
    }

    Ok(())
}
