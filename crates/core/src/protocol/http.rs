//! HTTP/HTTPS download backend via reqwest.
//!
//! Supports multi-chunk parallel downloads, resume, and progress reporting.

use std::path::{Path, PathBuf};

use futures::StreamExt;
use reqwest::header::{ACCEPT_RANGES, CONTENT_DISPOSITION, CONTENT_LENGTH, RANGE};
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;
use tracing::{debug, info, warn};

/// Progress update for a download.
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub speed_bytes_per_sec: u64,
}

/// Information gathered from a HEAD request.
#[derive(Debug, Clone)]
pub struct HeadInfo {
    pub content_length: Option<u64>,
    pub accepts_ranges: bool,
    pub filename: Option<String>,
}

pub struct HttpDownloader {
    client: reqwest::Client,
}

impl HttpDownloader {
    pub fn new(user_agent: &str) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .build()
            .expect("Failed to build reqwest client");
        Self { client }
    }

    /// Perform a HEAD request to get file info.
    pub async fn head(&self, url: &str) -> Result<HeadInfo, crate::Error> {
        let resp = self.client.head(url).send().await?.error_for_status()?;
        let headers = resp.headers();

        let content_length = headers
            .get(CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());

        let accepts_ranges = headers
            .get(ACCEPT_RANGES)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.contains("bytes"));

        let filename = headers
            .get(CONTENT_DISPOSITION)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_content_disposition_filename);

        Ok(HeadInfo {
            content_length,
            accepts_ranges,
            filename,
        })
    }

    /// Download a file with a single stream (no chunking).
    pub async fn download_single(
        &self,
        url: &str,
        dest: &Path,
        progress_tx: watch::Sender<DownloadProgress>,
    ) -> Result<u64, crate::Error> {
        let resp = self.client.get(url).send().await?.error_for_status()?;
        let total = resp.content_length();

        let mut file = tokio::fs::File::create(dest).await?;
        let mut stream = resp.bytes_stream();
        let mut downloaded: u64 = 0;
        let mut last_update = tokio::time::Instant::now();
        let mut speed_bytes: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(crate::Error::Http)?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            speed_bytes += chunk.len() as u64;

            let now = tokio::time::Instant::now();
            let elapsed = now.duration_since(last_update);
            if elapsed.as_millis() >= 500 {
                let speed = (speed_bytes as f64 / elapsed.as_secs_f64()) as u64;
                let _ = progress_tx.send(DownloadProgress {
                    bytes_downloaded: downloaded,
                    total_bytes: total,
                    speed_bytes_per_sec: speed,
                });
                speed_bytes = 0;
                last_update = now;
            }
        }

        file.flush().await?;

        let _ = progress_tx.send(DownloadProgress {
            bytes_downloaded: downloaded,
            total_bytes: total,
            speed_bytes_per_sec: 0,
        });

        info!("Download complete: {} bytes", downloaded);
        Ok(downloaded)
    }

    /// Download a file using multiple parallel chunks.
    pub async fn download_chunked(
        &self,
        url: &str,
        dest: &Path,
        temp_dir: &Path,
        total_size: u64,
        num_chunks: u32,
        progress_tx: watch::Sender<DownloadProgress>,
    ) -> Result<u64, crate::Error> {
        let chunk_size = total_size / num_chunks as u64;
        let mut handles = Vec::with_capacity(num_chunks as usize);

        // Per-chunk progress tracking
        let chunk_progress = std::sync::Arc::new(
            (0..num_chunks)
                .map(|_| std::sync::atomic::AtomicU64::new(0))
                .collect::<Vec<_>>(),
        );

        for i in 0..num_chunks {
            let start = i as u64 * chunk_size;
            let end = if i == num_chunks - 1 {
                total_size - 1
            } else {
                start + chunk_size - 1
            };

            let chunk_path = temp_dir.join(format!("chunk_{i}"));
            let client = self.client.clone();
            let url = url.to_string();
            let progress = chunk_progress.clone();

            let handle = tokio::spawn(async move {
                download_chunk(&client, &url, &chunk_path, start, end, i, &progress).await
            });
            handles.push(handle);
        }

        // Progress reporter task
        let progress_reporter = {
            let progress = chunk_progress.clone();
            let tx = progress_tx.clone();
            tokio::spawn(async move {
                let mut last_total: u64 = 0;
                let mut last_time = tokio::time::Instant::now();
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    let total_downloaded: u64 = progress
                        .iter()
                        .map(|a| a.load(std::sync::atomic::Ordering::Relaxed))
                        .sum();

                    let now = tokio::time::Instant::now();
                    let elapsed = now.duration_since(last_time);
                    let delta = total_downloaded.saturating_sub(last_total);
                    let speed = (delta as f64 / elapsed.as_secs_f64()) as u64;

                    let _ = tx.send(DownloadProgress {
                        bytes_downloaded: total_downloaded,
                        total_bytes: Some(total_size),
                        speed_bytes_per_sec: speed,
                    });

                    last_total = total_downloaded;
                    last_time = now;

                    if total_downloaded >= total_size {
                        break;
                    }
                }
            })
        };

        // Wait for all chunks
        for handle in handles {
            handle
                .await
                .map_err(|e| crate::Error::Other(e.to_string()))??;
        }
        progress_reporter.abort();

        // Reassemble chunks into final file
        debug!("Reassembling {} chunks into {:?}", num_chunks, dest);
        let mut dest_file = tokio::fs::File::create(dest).await?;
        for i in 0..num_chunks {
            let chunk_path = temp_dir.join(format!("chunk_{i}"));
            let chunk_data = tokio::fs::read(&chunk_path).await?;
            dest_file.write_all(&chunk_data).await?;
            tokio::fs::remove_file(&chunk_path).await?;
        }
        dest_file.flush().await?;

        let _ = progress_tx.send(DownloadProgress {
            bytes_downloaded: total_size,
            total_bytes: Some(total_size),
            speed_bytes_per_sec: 0,
        });

        info!(
            "Chunked download complete: {} bytes in {} chunks",
            total_size, num_chunks
        );
        Ok(total_size)
    }

    /// Resume a download from where it left off.
    pub async fn download_resume(
        &self,
        url: &str,
        dest: &Path,
        progress_tx: watch::Sender<DownloadProgress>,
    ) -> Result<u64, crate::Error> {
        let existing_size = if dest.exists() {
            tokio::fs::metadata(dest).await?.len()
        } else {
            0
        };

        if existing_size == 0 {
            return self.download_single(url, dest, progress_tx).await;
        }

        info!("Resuming download from byte {}", existing_size);

        let resp = self
            .client
            .get(url)
            .header(RANGE, format!("bytes={existing_size}-"))
            .send()
            .await?;

        if resp.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            warn!("Server doesn't support range requests, restarting download");
            return self.download_single(url, dest, progress_tx).await;
        }

        let resp = resp.error_for_status()?;
        let total = resp
            .content_length()
            .map(|remaining| remaining + existing_size);

        let mut file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(dest)
            .await?;

        let mut stream = resp.bytes_stream();
        let mut downloaded = existing_size;
        let mut last_update = tokio::time::Instant::now();
        let mut speed_bytes: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(crate::Error::Http)?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            speed_bytes += chunk.len() as u64;

            let now = tokio::time::Instant::now();
            let elapsed = now.duration_since(last_update);
            if elapsed.as_millis() >= 500 {
                let speed = (speed_bytes as f64 / elapsed.as_secs_f64()) as u64;
                let _ = progress_tx.send(DownloadProgress {
                    bytes_downloaded: downloaded,
                    total_bytes: total,
                    speed_bytes_per_sec: speed,
                });
                speed_bytes = 0;
                last_update = now;
            }
        }

        file.flush().await?;
        info!("Resume download complete: {} bytes total", downloaded);
        Ok(downloaded)
    }
}

/// Download a single chunk (byte range) to a temp file.
async fn download_chunk(
    client: &reqwest::Client,
    url: &str,
    chunk_path: &PathBuf,
    start: u64,
    end: u64,
    chunk_index: u32,
    progress: &[std::sync::atomic::AtomicU64],
) -> Result<(), crate::Error> {
    debug!("Chunk {chunk_index}: bytes {start}-{end}");

    let resp = client
        .get(url)
        .header(RANGE, format!("bytes={start}-{end}"))
        .send()
        .await?
        .error_for_status()?;

    let mut file = tokio::fs::File::create(chunk_path).await?;
    let mut stream = resp.bytes_stream();
    let mut chunk_downloaded: u64 = 0;

    while let Some(data) = stream.next().await {
        let data = data.map_err(crate::Error::Http)?;
        file.write_all(&data).await?;
        chunk_downloaded += data.len() as u64;
        progress[chunk_index as usize]
            .store(chunk_downloaded, std::sync::atomic::Ordering::Relaxed);
    }

    file.flush().await?;
    debug!("Chunk {chunk_index} complete: {chunk_downloaded} bytes");
    Ok(())
}

/// Parse filename from Content-Disposition header.
fn parse_content_disposition_filename(header: &str) -> Option<String> {
    // Try filename*= (RFC 5987)
    if let Some(pos) = header.find("filename*=") {
        let rest = &header[pos + 10..];
        if let Some(quote_start) = rest.find("''") {
            let name = &rest[quote_start + 2..];
            let name = name
                .split(';')
                .next()
                .unwrap_or(name)
                .trim()
                .trim_matches('"');
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    // Try filename=
    if let Some(pos) = header.find("filename=") {
        let rest = &header[pos + 9..];
        let name = rest
            .split(';')
            .next()
            .unwrap_or(rest)
            .trim()
            .trim_matches('"');
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_content_disposition() {
        assert_eq!(
            parse_content_disposition_filename("attachment; filename=\"test.zip\""),
            Some("test.zip".to_string())
        );
        assert_eq!(
            parse_content_disposition_filename("attachment; filename=test.zip"),
            Some("test.zip".to_string())
        );
        assert_eq!(parse_content_disposition_filename("inline"), None);
    }
}
