//! Download Coordinator — orchestrates downloads across all protocols.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{Mutex, broadcast, watch};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::bandwidth::BandwidthLimiter;
use crate::config::Config;
use crate::postprocess;
use crate::protocol::{Protocol, ResolvedDownload, UrlResolver};
use crate::protocol::http::{DownloadProgress, HttpDownloader};
use crate::protocol::hls::HlsDownloader;
use crate::protocol::dash::DashDownloader;
use crate::protocol::usenet::nntp::NntpServerConfig;
use crate::protocol::usenet::UsenetConfig;
use crate::protocol::usenet::UsenetDownloader;
use crate::queue::QueueStatus;
use crate::retry::{RetryOutcome, RetryPolicy, retry_with_policy};
use crate::storage::{DownloadRow, Storage};

/// Events broadcast to subscribers (WebSocket clients, webhooks, etc.)
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DownloadEvent {
    Added {
        id: String,
        url: String,
    },
    Progress {
        id: String,
        progress: DownloadProgress,
    },
    StatusChanged {
        id: String,
        status: String,
    },
    Completed {
        id: String,
    },
    Failed {
        id: String,
        error: String,
    },
    Removed {
        id: String,
    },
    /// Plugin-triggered notification for the UI.
    PluginNotification {
        plugin_id: String,
        title: String,
        message: String,
    },
    /// Captcha challenge that needs manual solving via the UI.
    CaptchaChallenge {
        id: String,
        plugin_id: String,
        download_id: String,
        image_url: String,
        captcha_type: String,
    },
    /// Captcha was solved by the user.
    CaptchaSolved {
        id: String,
    },
    /// Captcha timed out without being solved.
    CaptchaTimeout {
        id: String,
    },
    /// All queued downloads are complete (queue empty).
    QueueEmpty,
}

/// Tracks an active download task.
struct ActiveDownload {
    cancel_tx: tokio::sync::oneshot::Sender<()>,
    progress_rx: watch::Receiver<DownloadProgress>,
}

pub struct Coordinator {
    config: Arc<Mutex<Config>>,
    storage: Storage,
    bandwidth: BandwidthLimiter,
    retry_policy: Arc<Mutex<RetryPolicy>>,
    active: Arc<Mutex<HashMap<String, ActiveDownload>>>,
    event_tx: broadcast::Sender<DownloadEvent>,
    resolvers: Vec<Arc<dyn UrlResolver>>,
}

impl Coordinator {
    pub fn new(config: Config, storage: Storage) -> Self {
        let bandwidth = BandwidthLimiter::new(config.bandwidth.clone());
        let (event_tx, _) = broadcast::channel(256);

        let retry_policy = RetryPolicy::from(config.retry.clone());

        Self {
            config: Arc::new(Mutex::new(config)),
            storage,
            bandwidth,
            retry_policy: Arc::new(Mutex::new(retry_policy)),
            active: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
            resolvers: Vec::new(),
        }
    }

    /// Register URL resolvers (plugins, extractors). Called after construction.
    /// Resolvers are tried in order; first match wins.
    pub fn set_resolvers(&mut self, resolvers: Vec<Arc<dyn UrlResolver>>) {
        self.resolvers = resolvers;
    }

    /// Get a reference to the bandwidth limiter.
    pub fn bandwidth(&self) -> &BandwidthLimiter {
        &self.bandwidth
    }

    /// Subscribe to download events.
    pub fn subscribe(&self) -> broadcast::Receiver<DownloadEvent> {
        self.event_tx.subscribe()
    }

    /// Get the event sender (for wiring captcha manager, notifications, etc.)
    pub fn event_sender(&self) -> broadcast::Sender<DownloadEvent> {
        self.event_tx.clone()
    }

    /// Get a reference to storage.
    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    /// Get a snapshot of the current config.
    pub async fn config(&self) -> Config {
        self.config.lock().await.clone()
    }

    /// Update the config at runtime and propagate to subsystems.
    pub async fn update_config(&self, new_config: Config) {
        self.bandwidth.update_config(new_config.bandwidth.clone()).await;
        *self.retry_policy.lock().await = RetryPolicy::from(new_config.retry.clone());
        *self.config.lock().await = new_config;
    }

    /// Add a new download and start it if slots are available.
    pub async fn add_download(
        &self,
        url: &str,
        filename: Option<String>,
    ) -> Result<String, crate::Error> {
        self.add_download_with_options(url, filename, None, 0).await
    }

    /// Add a download with category and priority.
    pub async fn add_download_with_options(
        &self,
        url: &str,
        filename: Option<String>,
        category: Option<String>,
        priority: i32,
    ) -> Result<String, crate::Error> {
        // Duplicate detection: check if same URL is already queued/downloading
        let existing = self.storage.list_downloads().await?;
        if let Some(dup) = existing.iter().find(|d| {
            d.url == url
                && (d.status == "queued" || d.status == "downloading" || d.status == "paused")
        }) {
            info!("Duplicate download skipped: {} (existing: {})", url, dup.id);
            return Ok(dup.id.clone());
        }

        let id = Uuid::new_v4().to_string();

        // Try URL resolution: plugins → extractors → raw URL fallback
        let resolved = self.resolve_url(url).await;
        let (download_url, resolved_filename, protocol) = match &resolved {
            Some(r) => (
                r.url.clone(),
                r.filename.clone().or(filename.clone()),
                r.protocol.clone(),
            ),
            None => (url.to_string(), filename.clone(), detect_protocol(url)),
        };

        // Determine download directory (category-based subdirectory if set)
        let base_dir = self.storage.download_dir.to_string_lossy().to_string();
        let download_dir = match &category {
            Some(cat) if !cat.is_empty() => format!("{base_dir}/{cat}"),
            _ => base_dir,
        };

        let row = DownloadRow {
            id: id.clone(),
            url: download_url,
            protocol: protocol.as_str().to_string(),
            filename: resolved_filename,
            filesize: resolved.as_ref().and_then(|r| r.filesize),
            status: QueueStatus::Queued.as_str().to_string(),
            priority,
            package_id: category.clone(),
            plugin_id: None,
            download_dir: Some(download_dir),
            bytes_downloaded: 0,
            speed_current: 0,
            error_message: None,
            retry_count: 0,
            created_at: String::new(),
            started_at: None,
            completed_at: None,
        };

        self.storage.insert_download(&row).await?;
        let _ = self.event_tx.send(DownloadEvent::Added {
            id: id.clone(),
            url: url.to_string(),
        });

        info!("Download added: {id} — {url}");

        // Auto-start if slots available
        self.try_start_next().await?;

        Ok(id)
    }

    /// Try to start the next queued download if we have capacity.
    async fn try_start_next(&self) -> Result<(), crate::Error> {
        let active_count = {
            let active = self.active.lock().await;
            active.len() as u32
        };

        if active_count >= self.config.lock().await.max_concurrent_downloads {
            return Ok(());
        }

        let queued = self
            .storage
            .list_downloads_by_status(QueueStatus::Queued)
            .await?;
        if let Some(next) = queued.into_iter().next() {
            self.start_download(&next.id).await?;
        }

        Ok(())
    }

    /// Start downloading a specific item.
    async fn start_download(&self, id: &str) -> Result<(), crate::Error> {
        let row = self
            .storage
            .get_download(id)
            .await?
            .ok_or_else(|| crate::Error::Other(format!("Download not found: {id}")))?;

        self.storage
            .update_download_status(id, QueueStatus::Downloading)
            .await?;
        let _ = self.event_tx.send(DownloadEvent::StatusChanged {
            id: id.to_string(),
            status: "downloading".to_string(),
        });

        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
        let (progress_tx, progress_rx) = watch::channel(DownloadProgress {
            bytes_downloaded: 0,
            total_bytes: None,
            speed_bytes_per_sec: 0,
        });

        {
            let mut active = self.active.lock().await;
            active.insert(
                id.to_string(),
                ActiveDownload {
                    cancel_tx,
                    progress_rx: progress_rx.clone(),
                },
            );
        }

        // Spawn the download task with retry support
        let storage = self.storage.clone();
        let event_tx = self.event_tx.clone();
        let download_id = id.to_string();
        let url = row.url.clone();
        let protocol = row.protocol.clone();
        let download_dir = PathBuf::from(row.download_dir.as_deref().unwrap_or("downloads"));
        let temp_dir = self.storage.temp_dir.clone();
        let config = self.config.lock().await.clone();
        let user_agent = config.http.user_agent.clone();
        let max_chunks = config.http.max_chunks_per_download;
        let active = self.active.clone();
        let retry_policy = self.retry_policy.lock().await.clone();
        let bandwidth = self.bandwidth.clone();
        let pp_config = config.postprocessing.clone();

        tokio::spawn(async move {
            let mut original_progress_tx = Some(progress_tx);
            let mut original_cancel_rx = Some(cancel_rx);

            let result = retry_with_policy(&retry_policy, |attempt| {
                // Use the original progress_tx/cancel_rx on first attempt, create new ones for retries
                let ptx = original_progress_tx.take().unwrap_or_else(|| {
                    let (tx, _rx) = watch::channel(DownloadProgress {
                        bytes_downloaded: 0,
                        total_bytes: None,
                        speed_bytes_per_sec: 0,
                    });
                    tx
                });

                let crx = original_cancel_rx.take().unwrap_or_else(|| {
                    let (_tx, rx) = tokio::sync::oneshot::channel();
                    rx
                });

                let storage = &storage;
                let download_id = &download_id;
                let protocol = &protocol;
                let url = &url;
                let download_dir = &download_dir;
                let temp_dir = &temp_dir;
                let user_agent = &user_agent;
                let bandwidth = &bandwidth;
                let row_filename = &row.filename;

                async move {
                    if attempt > 0 {
                        let _ = storage
                            .update_download_error(download_id, "retrying", attempt)
                            .await;
                    }

                    let result = match protocol.as_str() {
                        "usenet" => {
                            run_usenet_download(
                                storage,
                                download_id,
                                download_dir,
                                row_filename.as_deref(),
                                ptx,
                            )
                            .await
                        }
                        "hls" => {
                            run_hls_download(
                                user_agent,
                                url,
                                download_dir,
                                row_filename.as_deref(),
                                ptx,
                                crx,
                            )
                            .await
                        }
                        "dash" => {
                            run_dash_download(
                                user_agent,
                                url,
                                download_dir,
                                row_filename.as_deref(),
                                ptx,
                                crx,
                            )
                            .await
                        }
                        _ => {
                            let http = HttpDownloader::new(user_agent, bandwidth.clone());
                            run_http_download(
                                &http,
                                url,
                                download_dir,
                                temp_dir,
                                row_filename.as_deref(),
                                max_chunks,
                                ptx,
                                crx,
                            )
                            .await
                        }
                    };

                    match result {
                        Ok(value) => RetryOutcome::Success(value),
                        Err(e) if e.to_string().contains("cancelled") => RetryOutcome::Abort(e),
                        Err(e) => {
                            warn!("Download attempt failed: {download_id} — {e}");
                            RetryOutcome::Retry(e)
                        }
                    }
                }
            })
            .await;

            match result {
                Ok((bytes, actual_path)) => {
                    let _ = storage
                        .update_download_progress(&download_id, bytes, 0)
                        .await;

                    // Run post-processing
                    if protocol == "usenet" {
                        let dir = actual_path.parent().unwrap_or(&actual_path);
                        if let Err(e) = postprocess::run_usenet_pipeline(dir, &pp_config).await {
                            warn!("Usenet post-processing failed for {download_id}: {e}");
                        }
                    } else if let Err(e) =
                        postprocess::run_pipeline(&actual_path, &pp_config).await
                    {
                        warn!("Post-processing failed for {download_id}: {e}");
                    }

                    let _ = storage
                        .update_download_status(&download_id, QueueStatus::Completed)
                        .await;
                    let _ = event_tx.send(DownloadEvent::Completed {
                        id: download_id.clone(),
                    });
                    info!("Download completed: {download_id}");
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    error!("Download failed: {download_id} — {error_msg}");
                    let _ = storage
                        .update_download_status(&download_id, QueueStatus::Failed)
                        .await;
                    let _ = storage
                        .update_download_error(&download_id, &error_msg, retry_policy.max_retries)
                        .await;
                    let _ = event_tx.send(DownloadEvent::Failed {
                        id: download_id.clone(),
                        error: error_msg,
                    });
                }
            }

            active.lock().await.remove(&download_id);
        });

        Ok(())
    }

    /// Pause an active download.
    pub async fn pause(&self, id: &str) -> Result<(), crate::Error> {
        let mut active = self.active.lock().await;
        if let Some(dl) = active.remove(id) {
            let _ = dl.cancel_tx.send(());
        }
        self.storage
            .update_download_status(id, QueueStatus::Paused)
            .await?;
        let _ = self.event_tx.send(DownloadEvent::StatusChanged {
            id: id.to_string(),
            status: "paused".to_string(),
        });
        Ok(())
    }

    /// Resume a paused download.
    pub async fn resume(&self, id: &str) -> Result<(), crate::Error> {
        self.storage
            .update_download_status(id, QueueStatus::Queued)
            .await?;
        self.try_start_next().await?;
        Ok(())
    }

    /// Cancel and remove a download.
    pub async fn cancel(&self, id: &str) -> Result<(), crate::Error> {
        let mut active = self.active.lock().await;
        if let Some(dl) = active.remove(id) {
            let _ = dl.cancel_tx.send(());
        }
        drop(active);
        self.storage.delete_download(id).await?;
        let _ = self
            .event_tx
            .send(DownloadEvent::Removed { id: id.to_string() });
        Ok(())
    }

    /// Get progress for an active download.
    pub async fn get_progress(&self, id: &str) -> Option<DownloadProgress> {
        let active = self.active.lock().await;
        active.get(id).map(|dl| dl.progress_rx.borrow().clone())
    }

    /// Get count of active downloads.
    pub async fn active_count(&self) -> usize {
        self.active.lock().await.len()
    }

    /// Get current aggregate speed.
    pub async fn total_speed(&self) -> u64 {
        let active = self.active.lock().await;
        active
            .values()
            .map(|dl| dl.progress_rx.borrow().speed_bytes_per_sec)
            .sum()
    }

    /// Update download priority (for queue reordering).
    pub async fn set_priority(&self, id: &str, priority: i32) -> Result<(), crate::Error> {
        self.storage.set_download_priority(id, priority).await
    }

    /// Recover downloads that were in-progress when the app crashed.
    /// Resets "downloading" status to "queued" so they can be restarted.
    pub async fn recover_downloads(&self) -> Result<u32, crate::Error> {
        let downloading = self
            .storage
            .list_downloads_by_status(QueueStatus::Downloading)
            .await?;

        let count = downloading.len() as u32;
        for d in &downloading {
            info!("Recovering interrupted download: {} ({})", d.id, d.url);
            self.storage
                .update_download_status(&d.id, QueueStatus::Queued)
                .await?;
        }

        if count > 0 {
            info!("Recovered {count} interrupted downloads — re-queued");
            self.try_start_next().await?;
        }

        Ok(count)
    }

    /// Try each registered URL resolver in order. Returns None if no resolver matches.
    async fn resolve_url(&self, url: &str) -> Option<ResolvedDownload> {
        for resolver in &self.resolvers {
            match resolver.resolve(url).await {
                Some(resolved) => {
                    info!(
                        "URL resolved: {url} → {} (protocol: {:?})",
                        resolved.url, resolved.protocol
                    );
                    return Some(resolved);
                }
                None => continue,
            }
        }
        None
    }
}

/// Run a single HTTP download to completion.
#[allow(clippy::too_many_arguments)]
async fn run_http_download(
    http: &HttpDownloader,
    url: &str,
    download_dir: &PathBuf,
    temp_dir: &PathBuf,
    filename: Option<&str>,
    max_chunks: u32,
    progress_tx: watch::Sender<DownloadProgress>,
    cancel_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<(u64, PathBuf), crate::Error> {
    // Get file info
    let head = http.head(url).await?;

    // Determine filename
    let fname = filename
        .map(String::from)
        .or(head.filename.clone())
        .unwrap_or_else(|| filename_from_url(url));

    let dest = download_dir.join(&fname);
    tokio::fs::create_dir_all(download_dir).await?;
    tokio::fs::create_dir_all(temp_dir).await?;

    // Choose strategy and race against cancellation
    let use_chunked = head.accepts_ranges
        && head.content_length.is_some_and(|s| s > 1024 * 1024)
        && max_chunks > 1;

    if use_chunked {
        let total = head.content_length.unwrap();
        let chunks = max_chunks.min((total / (512 * 1024)).max(1) as u32);
        info!("Using {chunks} chunks for {fname} ({total} bytes)");

        let chunk_dir = temp_dir.join(&fname);
        tokio::fs::create_dir_all(&chunk_dir).await?;

        tokio::select! {
            result = http.download_chunked(url, &dest, &chunk_dir, total, chunks, progress_tx) => result.map(|bytes| (bytes, dest)),
            _ = cancel_rx => {
                info!("Download cancelled: {url}");
                Err(crate::Error::Other("Download cancelled".into()))
            }
        }
    } else {
        info!("Using single-stream download for {fname}");
        tokio::select! {
            result = http.download_single(url, &dest, progress_tx) => result.map(|bytes| (bytes, dest)),
            _ = cancel_rx => {
                info!("Download cancelled: {url}");
                Err(crate::Error::Other("Download cancelled".into()))
            }
        }
    }
}

/// Run an HLS download to completion.
async fn run_hls_download(
    user_agent: &str,
    url: &str,
    download_dir: &PathBuf,
    filename: Option<&str>,
    progress_tx: watch::Sender<DownloadProgress>,
    cancel_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<(u64, PathBuf), crate::Error> {
    let fname = filename
        .map(String::from)
        .unwrap_or_else(|| filename_from_url(url).replace(".m3u8", ".ts").replace(".m3u", ".ts"));
    let dest = download_dir.join(&fname);
    tokio::fs::create_dir_all(download_dir).await?;

    let hls = HlsDownloader::new(user_agent, 8);
    tokio::select! {
        result = hls.download(url, &dest, progress_tx) => result.map(|bytes| (bytes, dest)),
        _ = cancel_rx => {
            info!("HLS download cancelled: {url}");
            Err(crate::Error::Other("Download cancelled".into()))
        }
    }
}

/// Run a DASH download to completion.
async fn run_dash_download(
    user_agent: &str,
    url: &str,
    download_dir: &PathBuf,
    filename: Option<&str>,
    progress_tx: watch::Sender<DownloadProgress>,
    cancel_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<(u64, PathBuf), crate::Error> {
    let fname = filename
        .map(String::from)
        .unwrap_or_else(|| filename_from_url(url).replace(".mpd", ".mp4"));
    let dest = download_dir.join(&fname);
    tokio::fs::create_dir_all(download_dir).await?;

    let dash = DashDownloader::new(user_agent, 8);
    tokio::select! {
        result = dash.download(url, &dest, progress_tx) => result.map(|bytes| (bytes, dest)),
        _ = cancel_rx => {
            info!("DASH download cancelled: {url}");
            Err(crate::Error::Other("Download cancelled".into()))
        }
    }
}

/// Run a Usenet download: load server configs from DB, download NZB segments via NNTP.
async fn run_usenet_download(
    storage: &Storage,
    download_id: &str,
    download_dir: &PathBuf,
    _filename: Option<&str>,
    progress_tx: watch::Sender<DownloadProgress>,
) -> Result<(u64, PathBuf), crate::Error> {
    // Load NNTP servers from database
    let server_rows = storage.list_usenet_servers().await?;
    if server_rows.is_empty() {
        return Err(crate::Error::Other(
            "No Usenet servers configured. Add a server in Settings → Usenet Servers.".into(),
        ));
    }

    let servers: Vec<NntpServerConfig> = server_rows
        .into_iter()
        .map(|r| NntpServerConfig {
            name: r.name,
            host: r.host,
            port: r.port,
            ssl: r.ssl,
            username: r.username,
            password: r.password,
            connections: r.connections,
            priority: r.priority,
        })
        .collect();

    let usenet = UsenetDownloader::new(UsenetConfig {
        servers,
        par2_repair: true,
        auto_unrar: true,
        delete_archives_after_extract: true,
    });

    tokio::fs::create_dir_all(download_dir).await?;

    // Load NZB data from download metadata
    let metadata = storage.get_download_metadata(download_id).await?;
    let nzb_data = metadata
        .as_deref()
        .and_then(|m| serde_json::from_str::<serde_json::Value>(m).ok())
        .and_then(|v| v.get("nzb_data")?.as_str().map(String::from));

    let nzb_data = match nzb_data {
        Some(data) => data,
        None => {
            return Err(crate::Error::Other(
                "No NZB data found in download metadata".into(),
            ));
        }
    };

    let _ = progress_tx.send(DownloadProgress {
        bytes_downloaded: 0,
        total_bytes: None,
        speed_bytes_per_sec: 0,
    });

    // Download all files from the NZB
    info!("Starting Usenet download to {:?}", download_dir);
    let results = usenet.download_nzb(&nzb_data, download_dir).await?;

    let total_bytes: u64 = results.iter().map(|r| r.bytes).sum();
    let total_ok: u32 = results.iter().map(|r| r.segments_ok).sum();
    let total_failed: u32 = results.iter().map(|r| r.segments_failed).sum();

    info!(
        "Usenet download complete: {} files, {} bytes, {}/{} segments OK",
        results.len(),
        total_bytes,
        total_ok,
        total_ok + total_failed
    );

    let _ = progress_tx.send(DownloadProgress {
        bytes_downloaded: total_bytes,
        total_bytes: Some(total_bytes),
        speed_bytes_per_sec: 0,
    });

    // Return the first file path or the directory
    let output_path = results
        .first()
        .map(|r| r.path.clone())
        .unwrap_or_else(|| download_dir.clone());

    Ok((total_bytes, output_path))
}

fn detect_protocol(url: &str) -> Protocol {
    let path = url.split('?').next().unwrap_or(url);
    if url.ends_with(".nzb") {
        Protocol::Usenet
    } else if path.ends_with(".m3u8") || path.ends_with(".m3u") {
        Protocol::Hls
    } else if path.ends_with(".mpd") {
        Protocol::Dash
    } else {
        Protocol::Http
    }
}

fn filename_from_url(url: &str) -> String {
    url.rsplit('/')
        .next()
        .and_then(|s| s.split('?').next())
        .filter(|s| !s.is_empty())
        .unwrap_or("download")
        .to_string()
}

impl Protocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::Http => "http",
            Protocol::Hls => "hls",
            Protocol::Dash => "dash",
            Protocol::Usenet => "usenet",
        }
    }
}
