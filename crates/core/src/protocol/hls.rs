//! HLS (HTTP Live Streaming) downloader.
//!
//! Parses m3u8 manifests and downloads segments in parallel, then concatenates
//! them into a single output file.

use std::path::Path;

use tokio::sync::watch;
use tracing::{debug, info, warn};

use super::http::DownloadProgress;

/// HLS downloader.
pub struct HlsDownloader {
    client: reqwest::Client,
    concurrent_segments: usize,
}

impl HlsDownloader {
    pub fn new(user_agent: &str, concurrent_segments: usize) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .build()
            .unwrap_or_default();
        Self {
            client,
            concurrent_segments,
        }
    }

    /// Download an HLS stream from a master or media playlist URL.
    pub async fn download(
        &self,
        manifest_url: &str,
        dest: &Path,
        progress_tx: watch::Sender<DownloadProgress>,
    ) -> Result<u64, crate::Error> {
        info!("Downloading HLS stream: {manifest_url}");

        let manifest_body = self.client.get(manifest_url).send().await?.text().await?;

        let parsed = m3u8_rs::parse_playlist_res(manifest_body.as_bytes())
            .map_err(|e| crate::Error::Other(format!("Failed to parse m3u8: {e:?}")))?;

        match parsed {
            m3u8_rs::Playlist::MasterPlaylist(master) => {
                // Select the best variant (highest bandwidth)
                let best = master
                    .variants
                    .iter()
                    .max_by_key(|v| v.bandwidth)
                    .ok_or_else(|| {
                        crate::Error::Other("No variants in HLS master playlist".into())
                    })?;

                let variant_url = resolve_url(manifest_url, &best.uri);
                info!(
                    "Selected HLS variant: bandwidth={}, uri={}",
                    best.bandwidth, best.uri
                );

                // Fetch the media playlist
                let media_body = self.client.get(&variant_url).send().await?.text().await?;

                let media_parsed =
                    m3u8_rs::parse_playlist_res(media_body.as_bytes()).map_err(|e| {
                        crate::Error::Other(format!("Failed to parse media playlist: {e:?}"))
                    })?;

                match media_parsed {
                    m3u8_rs::Playlist::MediaPlaylist(pl) => {
                        self.download_segments(&variant_url, &pl, dest, progress_tx)
                            .await
                    }
                    _ => Err(crate::Error::Other(
                        "Expected media playlist but got master".into(),
                    )),
                }
            }
            m3u8_rs::Playlist::MediaPlaylist(media) => {
                self.download_segments(manifest_url, &media, dest, progress_tx)
                    .await
            }
        }
    }

    /// Download all segments from a media playlist and concatenate to output.
    async fn download_segments(
        &self,
        playlist_url: &str,
        playlist: &m3u8_rs::MediaPlaylist,
        dest: &Path,
        progress_tx: watch::Sender<DownloadProgress>,
    ) -> Result<u64, crate::Error> {
        let segment_urls: Vec<String> = playlist
            .segments
            .iter()
            .map(|seg| resolve_url(playlist_url, &seg.uri))
            .collect();

        let total_segments = segment_urls.len();
        info!(
            "HLS: downloading {total_segments} segments ({} concurrent)",
            self.concurrent_segments
        );

        // Download segments in parallel batches
        let mut all_data: Vec<(usize, Vec<u8>)> = Vec::with_capacity(total_segments);
        let mut total_bytes: u64 = 0;

        for chunk in segment_urls.chunks(self.concurrent_segments) {
            let mut handles = Vec::new();

            for (offset, url) in chunk.iter().enumerate() {
                let client = self.client.clone();
                let url = url.clone();
                let idx = all_data.len() + offset;

                handles.push(tokio::spawn(async move {
                    let resp = client.get(&url).send().await?;
                    let bytes = resp.bytes().await?;
                    Ok::<(usize, Vec<u8>), reqwest::Error>((idx, bytes.to_vec()))
                }));
            }

            for handle in handles {
                match handle.await {
                    Ok(Ok((idx, data))) => {
                        total_bytes += data.len() as u64;
                        all_data.push((idx, data));

                        let _ = progress_tx.send(DownloadProgress {
                            bytes_downloaded: total_bytes,
                            total_bytes: None,
                            speed_bytes_per_sec: 0,
                        });
                    }
                    Ok(Err(e)) => {
                        warn!("HLS segment download failed: {e}");
                        return Err(crate::Error::Http(e));
                    }
                    Err(e) => {
                        warn!("HLS segment task failed: {e}");
                        return Err(crate::Error::Other(e.to_string()));
                    }
                }
            }

            debug!(
                "HLS: downloaded {}/{} segments",
                all_data.len(),
                total_segments
            );
        }

        // Sort by index and concatenate
        all_data.sort_by_key(|(idx, _)| *idx);

        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut output = tokio::fs::File::create(dest).await?;
        for (_, data) in &all_data {
            tokio::io::AsyncWriteExt::write_all(&mut output, data).await?;
        }

        info!("HLS: wrote {} bytes to {}", total_bytes, dest.display());

        let _ = progress_tx.send(DownloadProgress {
            bytes_downloaded: total_bytes,
            total_bytes: Some(total_bytes),
            speed_bytes_per_sec: 0,
        });

        Ok(total_bytes)
    }
}

/// Resolve a potentially relative URL against a base URL.
fn resolve_url(base: &str, relative: &str) -> String {
    if relative.starts_with("http://") || relative.starts_with("https://") {
        return relative.to_string();
    }

    if let Ok(base_url) = reqwest::Url::parse(base)
        && let Ok(resolved) = base_url.join(relative)
    {
        return resolved.to_string();
    }

    // Fallback: just join with the base directory
    if let Some(pos) = base.rfind('/') {
        format!("{}/{relative}", &base[..pos])
    } else {
        relative.to_string()
    }
}

/// Check if a URL looks like an HLS manifest.
pub fn is_hls_url(url: &str) -> bool {
    let path = url.split('?').next().unwrap_or(url);
    path.ends_with(".m3u8") || path.ends_with(".m3u")
}

#[async_trait::async_trait]
impl super::ProtocolBackend for HlsDownloader {
    fn protocol(&self) -> super::Protocol {
        super::Protocol::Hls
    }

    async fn download(
        &self,
        job: &super::DownloadJob,
        progress_tx: watch::Sender<DownloadProgress>,
        cancel_rx: tokio::sync::oneshot::Receiver<()>,
    ) -> Result<(u64, std::path::PathBuf), crate::Error> {
        let fname = job.filename.clone().unwrap_or_else(|| {
            job.url
                .rsplit('/')
                .next()
                .unwrap_or("stream")
                .to_string()
                .replace(".m3u8", ".ts")
                .replace(".m3u", ".ts")
        });
        let dest = job.download_dir.join(&fname);
        tokio::fs::create_dir_all(&job.download_dir).await?;

        tokio::select! {
            result = self.download(&job.url, &dest, progress_tx) => result.map(|bytes| (bytes, dest)),
            _ = cancel_rx => Err(crate::Error::Other("Download cancelled".into())),
        }
    }
}
