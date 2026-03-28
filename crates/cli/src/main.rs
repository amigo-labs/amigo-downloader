use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::sync::watch;
use tracing::debug;

use amigo_core::config::Config;
use amigo_core::coordinator::Coordinator;
use amigo_core::protocol::dash::{self, DashDownloader};
use amigo_core::protocol::hls::{self, HlsDownloader};
use amigo_core::protocol::http::{DownloadProgress, HttpDownloader};
use amigo_core::queue::QueueStatus;
use amigo_core::storage::Storage;
use amigo_extractors::youtube;

#[derive(Parser)]
#[command(name = "amigo-dl", about = "amigo-dl — fast, modular download manager")]
struct Cli {
    /// URLs to download directly (no subcommand needed)
    urls: Vec<String>,

    /// Output directory (for direct downloads)
    #[arg(short, long, default_value = ".")]
    output: String,

    /// Number of chunks per download (0 = auto)
    #[arg(short, long, default_value_t = 0)]
    chunks: u32,

    /// Enable verbose/debug logging
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Download URLs directly (explicit form of bare URL usage)
    Get {
        /// URLs to download
        urls: Vec<String>,
        /// Output directory
        #[arg(short, long, default_value = ".")]
        output: String,
        /// Number of chunks per download (0 = auto)
        #[arg(short, long, default_value_t = 0)]
        chunks: u32,
    },
    /// Add a download to the queue (URL, NZB, or DLC)
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
    /// Test a plugin: run spec file, or resolve a URL
    Test {
        /// Path to plugin file (.js or .ts)
        plugin: String,
        /// URL to resolve (if omitted, runs the .spec.ts/.spec.js file)
        url: Option<String>,
    },
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

/// Format bytes as human-readable string.
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Extract filename from URL.
fn filename_from_url(url: &str) -> String {
    url.rsplit('/')
        .next()
        .and_then(|s| s.split('?').next())
        .filter(|s| !s.is_empty())
        .unwrap_or("download")
        .to_string()
}

/// Detected download protocol for a URL.
enum DownloadProtocol {
    Http,
    Hls,
    Dash,
    YouTube,
}

fn detect_protocol(url: &str) -> DownloadProtocol {
    if youtube::is_youtube_url(url) {
        DownloadProtocol::YouTube
    } else if hls::is_hls_url(url) {
        DownloadProtocol::Hls
    } else if dash::is_dash_url(url) {
        DownloadProtocol::Dash
    } else {
        DownloadProtocol::Http
    }
}

/// Resolve a URL — extracts the actual download URL and filename.
/// For YouTube, this calls the innertube API. For plain URLs, uses HEAD.
struct ResolvedUrl {
    download_url: String,
    filename: String,
    filesize: Option<u64>,
    accepts_ranges: bool,
    protocol: DownloadProtocol,
}

async fn resolve_url(user_agent: &str, url: &str, pb: &ProgressBar) -> anyhow::Result<ResolvedUrl> {
    let protocol = detect_protocol(url);

    match protocol {
        DownloadProtocol::YouTube => {
            pb.set_message("Resolving YouTube video...");
            let client = reqwest::Client::builder()
                .user_agent(user_agent)
                .cookie_store(true)
                .build()?;
            let video = youtube::resolve(&client, url)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            pb.set_message(format!("{} [{}]", video.title, video.quality));

            Ok(ResolvedUrl {
                download_url: video.stream_url,
                filename: video.filename,
                filesize: video.filesize,
                // YouTube throttles chunked downloads
                accepts_ranges: false,
                protocol: DownloadProtocol::Http, // resolved to direct HTTP URL
            })
        }
        DownloadProtocol::Hls => {
            pb.set_message("HLS stream detected");
            let filename = filename_from_url(url)
                .replace(".m3u8", ".ts")
                .replace(".m3u", ".ts");
            Ok(ResolvedUrl {
                download_url: url.to_string(),
                filename,
                filesize: None,
                accepts_ranges: false,
                protocol: DownloadProtocol::Hls,
            })
        }
        DownloadProtocol::Dash => {
            pb.set_message("DASH stream detected");
            let filename = filename_from_url(url).replace(".mpd", ".mp4");
            Ok(ResolvedUrl {
                download_url: url.to_string(),
                filename,
                filesize: None,
                accepts_ranges: false,
                protocol: DownloadProtocol::Dash,
            })
        }
        DownloadProtocol::Http => {
            let http = HttpDownloader::new(user_agent);
            let head = http.head(url).await?;
            let filename = head
                .filename
                .clone()
                .unwrap_or_else(|| filename_from_url(url));

            Ok(ResolvedUrl {
                download_url: url.to_string(),
                filename,
                filesize: head.content_length,
                accepts_ranges: head.accepts_ranges,
                protocol: DownloadProtocol::Http,
            })
        }
    }
}

/// Run a direct download for a single URL with terminal progress.
async fn direct_download(
    user_agent: &str,
    url: &str,
    output_dir: &str,
    chunks_override: u32,
    pb: &ProgressBar,
) -> anyhow::Result<()> {
    let resolved = resolve_url(user_agent, url, pb).await?;

    let dest = PathBuf::from(output_dir).join(&resolved.filename);
    tokio::fs::create_dir_all(output_dir).await?;

    pb.set_message(resolved.filename.clone());

    if let Some(total) = resolved.filesize {
        pb.set_length(total);
    }

    let (progress_tx, progress_rx) = watch::channel(DownloadProgress {
        bytes_downloaded: 0,
        total_bytes: None,
        speed_bytes_per_sec: 0,
    });

    let download_url = resolved.download_url;
    let filename = resolved.filename;
    let dest_clone = dest.clone();
    let ua = user_agent.to_string();

    let download_handle = match resolved.protocol {
        DownloadProtocol::Hls => {
            debug!("Starting HLS download: {download_url}");
            tokio::spawn(async move {
                let hls = HlsDownloader::new(&ua, 8);
                hls.download(&download_url, &dest_clone, progress_tx)
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))
            })
        }
        DownloadProtocol::Dash => {
            debug!("Starting DASH download: {download_url}");
            tokio::spawn(async move {
                let dash = DashDownloader::new(&ua, 8);
                dash.download(&download_url, &dest_clone, progress_tx)
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))
            })
        }
        DownloadProtocol::Http | DownloadProtocol::YouTube => {
            // Decide chunk count
            let max_chunks = if chunks_override > 0 {
                chunks_override
            } else {
                Config::load_auto().http.max_chunks_per_download
            };

            let use_chunked = resolved.accepts_ranges
                && resolved.filesize.is_some_and(|s| s > 1024 * 1024)
                && max_chunks > 1;

            let temp_dir = PathBuf::from(output_dir).join(".amigo-tmp");
            tokio::fs::create_dir_all(&temp_dir).await?;

            if use_chunked {
                let total = resolved.filesize.unwrap();
                let num_chunks = max_chunks.min((total / (512 * 1024)).max(1) as u32);
                let chunk_dir = temp_dir.join(&filename);
                tokio::fs::create_dir_all(&chunk_dir).await?;

                tokio::spawn(async move {
                    let http = HttpDownloader::new(&ua);
                    http.download_chunked(
                        &download_url,
                        &dest_clone,
                        &chunk_dir,
                        total,
                        num_chunks,
                        progress_tx,
                    )
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))
                })
            } else {
                tokio::spawn(async move {
                    let http = HttpDownloader::new(&ua);
                    http.download_single(&download_url, &dest_clone, progress_tx)
                        .await
                        .map_err(|e| anyhow::anyhow!("{e}"))
                })
            }
        }
    };

    // Poll progress until download completes
    let filename_display = filename.clone();
    loop {
        if download_handle.is_finished() {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let p = progress_rx.borrow().clone();
        pb.set_position(p.bytes_downloaded);
        if let Some(total) = p.total_bytes {
            pb.set_length(total);
        }
        if p.speed_bytes_per_sec > 0 {
            pb.set_message(format!(
                "{} — {}/s",
                filename_display,
                format_bytes(p.speed_bytes_per_sec)
            ));
        }
    }

    let bytes = download_handle.await??;
    pb.set_position(bytes);
    pb.finish_with_message(format!("{} — {} done", filename, format_bytes(bytes)));

    // Clean up temp dir if empty
    let temp_dir = PathBuf::from(output_dir).join(".amigo-tmp");
    let _ = tokio::fs::remove_dir(&temp_dir).await;

    Ok(())
}

/// Run direct downloads for one or more URLs with progress bars.
async fn run_direct_downloads(urls: &[String], output: &str, chunks: u32) -> anyhow::Result<()> {
    if urls.is_empty() {
        anyhow::bail!("No URLs provided. Usage: amigo-dl <URL> [URL...]");
    }

    let config = Config::load_auto();
    let user_agent = config.http.user_agent.clone();

    let multi = MultiProgress::new();
    let style = ProgressStyle::with_template(
        "{spinner:.green} [{bar:40.cyan/dim}] {bytes}/{total_bytes} {msg}",
    )?
    .progress_chars("━╸─");

    for url in urls {
        // Check if a plugin from the registry could handle this URL
        check_plugin_suggestion(url).await;

        let pb = multi.add(ProgressBar::new(0));
        pb.set_style(style.clone());
        direct_download(&user_agent, url, output, chunks, &pb).await?;
    }

    Ok(())
}

/// Check the plugin registry for a plugin that can handle this URL.
/// Uses local cached index (instant), falls back to remote fetch if no cache.
async fn check_plugin_suggestion(url: &str) {
    // Skip URLs we already handle natively
    if youtube::is_youtube_url(url) || hls::is_hls_url(url) || dash::is_dash_url(url) {
        return;
    }

    let client = match reqwest::Client::builder()
        .user_agent("amigo-downloader")
        .timeout(std::time::Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    let config = amigo_plugin_runtime::registry::RegistryConfig::default();
    let index = match amigo_plugin_runtime::registry::load_index(&client, &config).await {
        Ok(idx) => idx,
        Err(_) => return,
    };

    if let Some(plugin) = amigo_plugin_runtime::registry::suggest_plugin_for_url(&index, url) {
        eprintln!(
            "{}",
            amigo_core::i18n::t_fmt(
                "plugin.install_hint",
                &[("name", &plugin.name), ("id", &plugin.id)]
            )
        );
        eprintln!();
    }
}

fn init_tracing(verbose: bool) {
    use tracing_subscriber::EnvFilter;

    let default_level = if verbose { "debug" } else { "warn" };

    // RUST_LOG env takes precedence
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // plugins test always runs with verbose logging
    let verbose = cli.verbose
        || matches!(
            &cli.command,
            Some(Commands::Plugins {
                action: PluginAction::Test { .. }
            })
        );
    init_tracing(verbose);

    // Initialize i18n — detect system language, load locale
    let lang = amigo_core::i18n::detect_system_lang();
    amigo_core::i18n::init(&lang, std::path::Path::new("locales"));

    // Bare URLs without subcommand → direct download
    if cli.command.is_none() {
        if cli.urls.is_empty() {
            Cli::parse_from(["amigo-dl", "--help"]);
        }
        return run_direct_downloads(&cli.urls, &cli.output, cli.chunks).await;
    }

    match cli.command.unwrap() {
        Commands::Get {
            urls,
            output,
            chunks,
        } => {
            run_direct_downloads(&urls, &output, chunks).await?;
        }

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
                    println!(
                        "  {} ({} segments, {} bytes)",
                        file.filename(),
                        file.segments.len(),
                        file.total_bytes()
                    );
                }
                for file in &nzb.files {
                    let id = coord
                        .add_download(&format!("nzb://{}", file.filename()), Some(file.filename()))
                        .await?;
                    println!("  Queued: {} → {id}", file.filename());
                }
                println!("NZB import complete ({} files queued).", nzb.files.len());
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
                    let pct = d.filesize.map(|s| {
                        if s > 0 {
                            d.bytes_downloaded * 100 / s
                        } else {
                            0
                        }
                    });
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
            let queued = coord
                .storage()
                .list_downloads_by_status(QueueStatus::Queued)
                .await?;
            if queued.is_empty() {
                println!("Queue is empty.");
            } else {
                for (i, d) in queued.iter().enumerate() {
                    println!(
                        "{}. {} — {}",
                        i + 1,
                        d.filename.as_deref().unwrap_or(&d.url),
                        d.id
                    );
                }
            }
        }

        Commands::Status => {
            let coord = init_coordinator()?;
            let active = coord.active_count().await;
            let speed = coord.total_speed().await;
            let queued = coord.storage().count_by_status(QueueStatus::Queued).await?;
            let completed = coord
                .storage()
                .count_by_status(QueueStatus::Completed)
                .await?;
            let failed = coord.storage().count_by_status(QueueStatus::Failed).await?;

            println!("amigo-dl v{}", amigo_core::updater::CURRENT_VERSION);
            println!(
                "Active: {active}  Queued: {queued}  Completed: {completed}  Failed: {failed}"
            );
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
                    let mut json = serde_json::to_value(&config)?;
                    let pointer = format!("/{}", key.replace('.', "/"));
                    if let Some(field) = json.pointer_mut(&pointer) {
                        *field = if let Ok(n) = value.parse::<u64>() {
                            serde_json::Value::Number(n.into())
                        } else if let Ok(b) = value.parse::<bool>() {
                            serde_json::Value::Bool(b)
                        } else {
                            serde_json::Value::String(value.clone())
                        };
                        let updated: Config = serde_json::from_value(json)?;
                        updated.save(std::path::Path::new("config.toml"))?;
                        println!("{key} = {value} (saved)");
                    } else {
                        println!("Key not found: {key}. Available keys:");
                        fn print_keys(prefix: &str, val: &serde_json::Value) {
                            match val {
                                serde_json::Value::Object(map) => {
                                    for (k, v) in map {
                                        let full = if prefix.is_empty() {
                                            k.clone()
                                        } else {
                                            format!("{prefix}.{k}")
                                        };
                                        print_keys(&full, v);
                                    }
                                }
                                _ => println!("  {prefix}"),
                            }
                        }
                        print_keys("", &serde_json::to_value(&config)?);
                    }
                }
            }
        }

        Commands::Plugins { action } => match action {
            PluginAction::Test { plugin, url } => {
                use amigo_plugin_runtime::loader::PluginLoader;
                use amigo_plugin_runtime::sandbox::SandboxLimits;

                let path = std::path::Path::new(&plugin);
                if !path.exists() {
                    anyhow::bail!("Plugin file not found: {plugin}");
                }

                let plugin_dir = path.parent().unwrap_or(std::path::Path::new("."));
                let loader = PluginLoader::new(plugin_dir.to_path_buf(), SandboxLimits::default());
                loader
                    .discover()
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;

                let plugins = loader.list_plugins().await;
                if plugins.is_empty() {
                    anyhow::bail!("No plugin found in {}", plugin);
                }

                let meta = &plugins[0];
                println!("Plugin: {} v{} ({})", meta.name, meta.version, meta.id);
                println!("Pattern: {}", meta.url_pattern);

                if let Some(url) = url {
                    // Mode 1: Resolve a URL
                    let re = regex::Regex::new(&meta.url_pattern)?;
                    if !re.is_match(&url) {
                        println!("\nURL does not match plugin's urlPattern.");
                        return Ok(());
                    }

                    println!("\nResolving: {url}");
                    match loader.resolve(&meta.id, &url).await {
                        Ok(pkg) => {
                            println!("\n{}", serde_json::to_string_pretty(&pkg)?);
                        }
                        Err(e) => {
                            println!("\nError: {e}");
                        }
                    }
                } else {
                    // Mode 2: Run spec file
                    println!();
                    match loader.run_spec(&meta.id).await {
                        Ok(results) => {
                            for r in &results.results {
                                if r.passed {
                                    println!("  PASS  {}", r.name);
                                } else {
                                    println!(
                                        "  FAIL  {} — {}",
                                        r.name,
                                        r.error.as_deref().unwrap_or("?")
                                    );
                                }
                            }
                            println!();
                            println!("{} passed, {} failed", results.passed, results.failed);
                            if results.failed > 0 {
                                std::process::exit(1);
                            }
                        }
                        Err(e) => {
                            anyhow::bail!("{e}");
                        }
                    }
                }
            }
            PluginAction::List | PluginAction::Enable { .. } | PluginAction::Login { .. } => {
                println!("Plugin management requires the server. Use: amigo-dl serve");
            }
            PluginAction::Update { .. }
            | PluginAction::Install { .. }
            | PluginAction::Search { .. } => {
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

                match amigo_core::updater::check_for_update(&client, &config.update.github_repo)
                    .await
                {
                    Ok(amigo_core::updater::CoreUpdateStatus::UpdateAvailable {
                        current,
                        latest,
                        release_notes,
                        ..
                    }) => {
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
                let client = reqwest::Client::builder()
                    .user_agent("amigo-downloader")
                    .build()?;
                let config = Config::load_auto();

                let status =
                    amigo_core::updater::check_for_update(&client, &config.update.github_repo)
                        .await?;

                match status {
                    amigo_core::updater::CoreUpdateStatus::UpdateAvailable {
                        current,
                        latest,
                        download_url,
                        sha256_url,
                        ..
                    } => {
                        if !yes {
                            println!("Update available: {current} → {latest}");
                            println!("Run with --yes to apply.");
                            return Ok(());
                        }
                        println!("Updating {current} → {latest}...");
                        amigo_core::updater::download_and_apply(
                            &client,
                            &download_url,
                            sha256_url.as_deref(),
                        )
                        .await?;
                        println!("Update applied! Restart amigo-dl to use v{latest}.");
                    }
                    amigo_core::updater::CoreUpdateStatus::UpToDate => {
                        println!(
                            "Already up to date (v{}).",
                            amigo_core::updater::CURRENT_VERSION
                        );
                    }
                }
            }
        },

        Commands::Serve { port, bind } => {
            let addr = format!("{bind}:{port}");
            println!("Starting amigo-server on {addr}...");
            println!("For the full server with plugins and web UI, use the `amigo-server` binary.");
            println!("This lite-mode serves only the REST API.");

            let coord = init_coordinator()?;
            let listener = tokio::net::TcpListener::bind(&addr).await?;

            let _state = std::sync::Arc::new(coord);
            let app = axum::Router::new()
                .route("/api/v1/status", axum::routing::get(|| async {
                    axum::Json(serde_json::json!({"status": "ok", "version": env!("CARGO_PKG_VERSION"), "mode": "cli"}))
                }));

            println!("Listening on {addr}");
            axum::serve(listener, app).await?;
        }
    }

    Ok(())
}
