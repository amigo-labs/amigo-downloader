use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "amigo", about = "amigo-downloader — fast, modular download manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a download (URL, NZB, torrent, magnet, or DLC)
    Add {
        /// URL to download
        url: Option<String>,
        /// Import NZB file
        #[arg(long)]
        nzb: Option<String>,
        /// Import torrent file
        #[arg(long)]
        torrent: Option<String>,
        /// Magnet link
        #[arg(long)]
        magnet: Option<String>,
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
    Update {
        /// Plugin ID (all if omitted)
        id: Option<String>,
    },
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Update { action } => match action {
            UpdateAction::Check => {
                let client = reqwest::Client::builder()
                    .user_agent("amigo-downloader")
                    .build()?;
                let config = amigo_core::config::Config::default();

                println!("Checking for updates...");

                // Check core update
                match amigo_core::updater::check_for_update(&client, &config.update.github_repo).await {
                    Ok(amigo_core::updater::CoreUpdateStatus::UpdateAvailable { current, latest, .. }) => {
                        println!("Core update available: {current} → {latest}");
                    }
                    Ok(amigo_core::updater::CoreUpdateStatus::UpToDate) => {
                        println!("Core is up to date (v{})", amigo_core::updater::CURRENT_VERSION);
                    }
                    Err(e) => {
                        println!("Could not check for core updates: {e}");
                    }
                }

                println!("Done.");
            }
            UpdateAction::Apply { yes } => {
                if !yes {
                    println!("Use --yes to confirm the update.");
                    return Ok(());
                }
                println!("Self-update not yet fully implemented.");
            }
        },
        _ => {
            todo!("Implement remaining CLI commands")
        }
    }

    Ok(())
}
