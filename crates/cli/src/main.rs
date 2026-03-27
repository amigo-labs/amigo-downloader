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
    List,
    Enable { id: String },
    Login { id: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _cli = Cli::parse();
    todo!("Implement CLI commands")
}
