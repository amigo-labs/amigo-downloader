//! Download Coordinator — orchestrates downloads across all protocols.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{Mutex, broadcast, watch};
use tracing::{error, info};
use uuid::Uuid;

use crate::bandwidth::BandwidthLimiter;
use crate::config::Config;
use crate::protocol::Protocol;
use crate::protocol::http::{DownloadProgress, HttpDownloader};
use crate::queue::QueueStatus;
use crate::storage::{DownloadRow, Storage};

/// Events broadcast to subscribers (WebSocket clients, etc.)
#[derive(Debug, Clone)]
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
}

/// Tracks an active download task.
struct ActiveDownload {
    cancel_tx: tokio::sync::oneshot::Sender<()>,
    progress_rx: watch::Receiver<DownloadProgress>,
}

pub struct Coordinator {
    config: Config,
    storage: Storage,
    http: HttpDownloader,
    bandwidth: BandwidthLimiter,
    active: Arc<Mutex<HashMap<String, ActiveDownload>>>,
    event_tx: broadcast::Sender<DownloadEvent>,
}

impl Coordinator {
    pub fn new(config: Config, storage: Storage) -> Self {
        let http = HttpDownloader::new(&config.http.user_agent);
        let bandwidth = BandwidthLimiter::new(config.bandwidth.clone());
        let (event_tx, _) = broadcast::channel(256);

        Self {
            config,
            storage,
            http,
            bandwidth,
            active: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
        }
    }

    /// Subscribe to download events.
    pub fn subscribe(&self) -> broadcast::Receiver<DownloadEvent> {
        self.event_tx.subscribe()
    }

    /// Get a reference to storage.
    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    /// Get a reference to config.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Add a new download and start it if slots are available.
    pub async fn add_download(
        &self,
        url: &str,
        filename: Option<String>,
    ) -> Result<String, crate::Error> {
        let id = Uuid::new_v4().to_string();
        let protocol = detect_protocol(url);

        let row = DownloadRow {
            id: id.clone(),
            url: url.to_string(),
            protocol: protocol.as_str().to_string(),
            filename,
            filesize: None,
            status: QueueStatus::Queued.as_str().to_string(),
            priority: 0,
            package_id: None,
            plugin_id: None,
            download_dir: Some(self.storage.download_dir.to_string_lossy().to_string()),
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

        if active_count >= self.config.max_concurrent_downloads {
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

        // Spawn the download task
        let storage = self.storage.clone();
        let event_tx = self.event_tx.clone();
        let download_id = id.to_string();
        let url = row.url.clone();
        let download_dir = PathBuf::from(row.download_dir.as_deref().unwrap_or("downloads"));
        let temp_dir = self.storage.temp_dir.clone();
        let http = HttpDownloader::new(&self.config.http.user_agent);
        let max_chunks = self.config.http.max_chunks_per_download;
        let active = self.active.clone();

        tokio::spawn(async move {
            let result = run_http_download(
                &http,
                &url,
                &download_dir,
                &temp_dir,
                row.filename.as_deref(),
                max_chunks,
                progress_tx,
                cancel_rx,
            )
            .await;

            match result {
                Ok(bytes) => {
                    let _ = storage
                        .update_download_progress(&download_id, bytes, 0)
                        .await;
                    let _ = storage
                        .update_download_status(&download_id, QueueStatus::Completed)
                        .await;
                    let _ = event_tx.send(DownloadEvent::Completed {
                        id: download_id.clone(),
                    });
                    info!("Download completed: {download_id}");
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    error!("Download failed: {download_id} — {err_msg}");
                    let _ = storage
                        .update_download_status(&download_id, QueueStatus::Failed)
                        .await;
                    let _ = storage
                        .update_download_error(&download_id, &err_msg, 0)
                        .await;
                    let _ = event_tx.send(DownloadEvent::Failed {
                        id: download_id.clone(),
                        error: err_msg,
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
}

/// Run a single HTTP download to completion.
async fn run_http_download(
    http: &HttpDownloader,
    url: &str,
    download_dir: &PathBuf,
    temp_dir: &PathBuf,
    filename: Option<&str>,
    max_chunks: u32,
    progress_tx: watch::Sender<DownloadProgress>,
    cancel_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<u64, crate::Error> {
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
            result = http.download_chunked(url, &dest, &chunk_dir, total, chunks, progress_tx) => result,
            _ = cancel_rx => {
                info!("Download cancelled: {url}");
                Err(crate::Error::Other("Download cancelled".into()))
            }
        }
    } else {
        info!("Using single-stream download for {fname}");
        tokio::select! {
            result = http.download_single(url, &dest, progress_tx) => result,
            _ = cancel_rx => {
                info!("Download cancelled: {url}");
                Err(crate::Error::Other("Download cancelled".into()))
            }
        }
    }
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
