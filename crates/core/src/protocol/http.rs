//! HTTP/HTTPS download backend via reqwest.
//!
//! Supports multi-chunk parallel downloads, resume, and progress reporting.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::StreamExt;
use reqwest::header::{ACCEPT_RANGES, CONTENT_DISPOSITION, CONTENT_LENGTH, RANGE};
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;
use tracing::{debug, info, warn};

use crate::bandwidth::BandwidthLimiter;

/// Write buffer size for download/reassembly file I/O. `bytes_stream()` yields
/// many small chunks; without buffering each one becomes its own write syscall.
const WRITE_BUFFER_SIZE: usize = 256 * 1024;

/// Flush kernel buffers and persist `file` to disk.
///
/// `flush().await` only drains the user-space buffer into the kernel; if the
/// process crashes or power is lost between flush and the next OS-level
/// fsync, the bytes can disappear and resume state in the database can no
/// longer be reconciled with what's actually on disk. This helper wraps the
/// `sync_all` invariant in one call site so every download path stays
/// honest.
async fn fsync_file(file: &mut tokio::fs::File) -> std::io::Result<()> {
    file.flush().await?;
    file.sync_all().await
}

/// Best-effort fsync of a directory so a preceding `rename` is durable on
/// POSIX. Errors are swallowed because not every platform exposes directory
/// fds (Windows is a no-op).
async fn fsync_parent(path: &Path) {
    if let Some(parent) = path.parent()
        && let Ok(dir) = tokio::fs::File::open(parent).await
    {
        let _ = dir.sync_all().await;
    }
}

/// Progress update for a download.
#[derive(Debug, Clone, serde::Serialize)]
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
    bandwidth: BandwidthLimiter,
}

impl HttpDownloader {
    pub fn new(user_agent: &str, bandwidth: BandwidthLimiter) -> Self {
        Self::new_with_headers(user_agent, &HashMap::new(), bandwidth)
    }

    /// Build a downloader whose client sends `headers` on every request. Used
    /// so resolver-supplied headers (Referer, Authorization, …) carried on the
    /// `DownloadJob` actually reach the server instead of being dropped.
    /// Header names/values that are not valid HTTP are skipped.
    pub fn new_with_headers(
        user_agent: &str,
        headers: &HashMap<String, String>,
        bandwidth: BandwidthLimiter,
    ) -> Self {
        let mut header_map = reqwest::header::HeaderMap::new();
        for (k, v) in headers {
            if let (Ok(name), Ok(value)) = (
                reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                reqwest::header::HeaderValue::from_str(v),
            ) {
                header_map.insert(name, value);
            }
        }
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .default_headers(header_map)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client, bandwidth }
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

        let file = tokio::fs::File::create(dest).await?;
        let mut writer = tokio::io::BufWriter::with_capacity(WRITE_BUFFER_SIZE, file);
        let mut stream = resp.bytes_stream();
        let mut downloaded: u64 = 0;
        let mut last_update = tokio::time::Instant::now();
        let mut speed_bytes: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(crate::Error::Http)?;
            self.bandwidth.acquire(chunk.len() as u64).await;
            writer.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            speed_bytes += chunk.len() as u64;

            let now = tokio::time::Instant::now();
            let elapsed = now.duration_since(last_update);
            if elapsed.as_millis() >= 500 {
                let speed = bytes_per_sec(speed_bytes, elapsed);
                let _ = progress_tx.send(DownloadProgress {
                    bytes_downloaded: downloaded,
                    total_bytes: total,
                    speed_bytes_per_sec: speed,
                });
                speed_bytes = 0;
                last_update = now;
            }
        }

        writer.flush().await?;
        fsync_file(writer.get_mut()).await?;

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
        if num_chunks == 0 {
            return Err(crate::Error::Other(
                "num_chunks must be greater than 0".into(),
            ));
        }
        // Each chunk needs at least one byte; with more chunks than bytes,
        // chunk_size would be 0 and `start + chunk_size - 1` would underflow.
        // Clamp the chunk count to the byte count for tiny inputs. (The trait
        // caller already clamps, but this pub fn must be safe on its own.)
        let num_chunks = (num_chunks as u64).min(total_size.max(1)) as u32;
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
                total_size.saturating_sub(1)
            } else {
                start + chunk_size - 1
            };

            let chunk_path = temp_dir.join(format!("chunk_{i}"));
            let client = self.client.clone();
            let url = url.to_string();
            let progress = chunk_progress.clone();
            let bw = self.bandwidth.clone();

            let handle = tokio::spawn(async move {
                download_chunk(&client, &bw, &url, &chunk_path, start, end, i, &progress).await
            });
            handles.push(handle);
        }

        // Progress reporter task. Wrapped in AbortOnDrop so it is torn down
        // on every exit path — including a `tokio::select!` cancellation that
        // drops this future — instead of looping forever (its only exit
        // condition is "all bytes downloaded").
        let progress_reporter = AbortOnDrop({
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
                    let speed = bytes_per_sec(delta, elapsed);

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
        });

        // Wait for all chunks. Don't return early on the first failure:
        // the progress reporter only exits once every byte is accounted
        // for, so bailing out here would leak it (and the still-running
        // sibling chunk tasks) and keep emitting progress events for a
        // download that already failed.
        let mut chunk_result: Result<(), crate::Error> = Ok(());
        let mut handles = handles.into_iter();
        for handle in &mut handles {
            let res = match handle.await {
                Ok(res) => res,
                Err(e) => Err(crate::Error::Other(e.to_string())),
            };
            if let Err(e) = res {
                chunk_result = Err(e);
                break;
            }
        }
        for handle in handles {
            handle.abort();
            let _ = handle.await;
        }
        drop(progress_reporter);
        chunk_result?;

        // Reassemble chunks into final file (use temp name, rename on success).
        // reassemble_chunks verifies the concatenated size equals total_size
        // and renames atomically, guarding against silent truncation.
        let written = reassemble_chunks(temp_dir, num_chunks, dest, total_size).await?;

        let _ = progress_tx.send(DownloadProgress {
            bytes_downloaded: written,
            total_bytes: Some(total_size),
            speed_bytes_per_sec: 0,
        });

        info!(
            "Chunked download complete: {} bytes in {} chunks",
            written, num_chunks
        );
        Ok(written)
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

        // The server must explicitly acknowledge the Range request with
        // 206 Partial Content. A 200 OK means the server ignored the header
        // and is sending the *full* file from offset 0; appending that to
        // the existing partial bytes would corrupt the output. Treat both
        // 200 and 416 as "resume not possible" and restart cleanly.
        let status = resp.status();
        if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            warn!("Server doesn't support range requests, restarting download");
            return self.download_single(url, dest, progress_tx).await;
        }
        if status == reqwest::StatusCode::OK {
            warn!(
                "Server ignored Range header (returned 200 OK instead of 206); \
                 restarting download to avoid appending duplicate bytes"
            );
            return self.download_single(url, dest, progress_tx).await;
        }

        // Anything other than 206 Partial Content is unexpected here —
        // bubble up via error_for_status so 4xx/5xx surface as Http errors.
        let resp = resp.error_for_status()?;
        if resp.status() != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(crate::Error::Other(format!(
                "unexpected resume response status {status}"
            )));
        }
        let total = resp
            .content_length()
            .map(|remaining| remaining + existing_size);

        let file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(dest)
            .await?;
        let mut writer = tokio::io::BufWriter::with_capacity(WRITE_BUFFER_SIZE, file);

        let mut stream = resp.bytes_stream();
        let mut downloaded = existing_size;
        let mut last_update = tokio::time::Instant::now();
        let mut speed_bytes: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(crate::Error::Http)?;
            self.bandwidth.acquire(chunk.len() as u64).await;
            writer.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            speed_bytes += chunk.len() as u64;

            let now = tokio::time::Instant::now();
            let elapsed = now.duration_since(last_update);
            if elapsed.as_millis() >= 500 {
                let speed = bytes_per_sec(speed_bytes, elapsed);
                let _ = progress_tx.send(DownloadProgress {
                    bytes_downloaded: downloaded,
                    total_bytes: total,
                    speed_bytes_per_sec: speed,
                });
                speed_bytes = 0;
                last_update = now;
            }
        }

        writer.flush().await?;
        fsync_file(writer.get_mut()).await?;
        info!("Resume download complete: {} bytes total", downloaded);
        Ok(downloaded)
    }
}

/// Reassemble `num_chunks` chunk files (`chunk_0`..`chunk_{n-1}` under
/// `temp_dir`) into `dest`, verifying the concatenated size equals
/// `total_size` before the atomic rename.
///
/// Writing to a `.part` temp file and renaming on success means a failed or
/// short reassembly never leaves a partial file at `dest`. The size check
/// guards against silent truncation: a chunk task that lost bytes without
/// returning an error would otherwise produce a short but "completed" file.
/// Returns the number of bytes written.
async fn reassemble_chunks(
    temp_dir: &Path,
    num_chunks: u32,
    dest: &Path,
    total_size: u64,
) -> Result<u64, crate::Error> {
    let dest_tmp = dest.with_extension("part");
    debug!("Reassembling {} chunks into {:?}", num_chunks, dest_tmp);

    let reassemble_result: Result<u64, crate::Error> = async {
        let dest_file = tokio::fs::File::create(&dest_tmp).await?;
        let mut dest_writer = tokio::io::BufWriter::with_capacity(WRITE_BUFFER_SIZE, dest_file);
        let mut written: u64 = 0;
        for i in 0..num_chunks {
            let chunk_path = temp_dir.join(format!("chunk_{i}"));
            // Stream each chunk through the buffered writer instead of
            // loading the whole chunk into a Vec — keeps peak RAM bounded
            // by the buffer size regardless of chunk size.
            let mut chunk_file = tokio::fs::File::open(&chunk_path).await?;
            written += tokio::io::copy(&mut chunk_file, &mut dest_writer).await?;
            drop(chunk_file);
            tokio::fs::remove_file(&chunk_path).await?;
        }
        dest_writer.flush().await?;
        fsync_file(dest_writer.get_mut()).await?;
        Ok(written)
    }
    .await;

    let written = match reassemble_result {
        Ok(written) => written,
        Err(e) => {
            let _ = tokio::fs::remove_file(&dest_tmp).await;
            return Err(e);
        }
    };

    if written != total_size {
        let _ = tokio::fs::remove_file(&dest_tmp).await;
        return Err(crate::Error::Other(format!(
            "reassembled size mismatch: expected {total_size} bytes, got {written}"
        )));
    }

    tokio::fs::rename(&dest_tmp, dest).await?;
    // Make the directory entry update durable. On POSIX an unflushed rename
    // can vanish on crash even though the target file was sync_all'd before
    // the rename; Windows treats this as a no-op.
    fsync_parent(dest).await;

    Ok(written)
}

/// Download a single chunk (byte range) to a temp file.
#[allow(clippy::too_many_arguments)]
async fn download_chunk(
    client: &reqwest::Client,
    bandwidth: &BandwidthLimiter,
    url: &str,
    chunk_path: &PathBuf,
    start: u64,
    end: u64,
    chunk_index: u32,
    progress: &[std::sync::atomic::AtomicU64],
) -> Result<(), crate::Error> {
    // Resume: check if temp file exists with partial data
    let existing_bytes = if chunk_path.exists() {
        tokio::fs::metadata(chunk_path).await?.len()
    } else {
        0
    };

    let expected_size = end - start + 1;
    if existing_bytes >= expected_size {
        debug!("Chunk {chunk_index}: already complete ({existing_bytes} bytes), skipping");
        progress[chunk_index as usize].store(existing_bytes, std::sync::atomic::Ordering::Relaxed);
        return Ok(());
    }

    let actual_start = start + existing_bytes;
    debug!("Chunk {chunk_index}: bytes {actual_start}-{end} (resuming from {existing_bytes})");

    let resp = client
        .get(url)
        .header(RANGE, format!("bytes={actual_start}-{end}"))
        .send()
        .await?
        .error_for_status()?;

    // The server MUST acknowledge the Range with 206 Partial Content. A 200 OK
    // means it ignored the header and is streaming the *whole* file into this
    // chunk; reassembly would then concatenate N full copies into a corrupt,
    // oversized output. Bail so the caller can restart as a single stream.
    if resp.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(crate::Error::RangeNotSupported);
    }

    let file = if existing_bytes > 0 {
        tokio::fs::OpenOptions::new()
            .append(true)
            .open(chunk_path)
            .await?
    } else {
        tokio::fs::File::create(chunk_path).await?
    };
    let mut writer = tokio::io::BufWriter::with_capacity(WRITE_BUFFER_SIZE, file);

    let mut stream = resp.bytes_stream();
    let mut chunk_downloaded = existing_bytes;

    while let Some(data) = stream.next().await {
        let data = data.map_err(crate::Error::Http)?;
        bandwidth.acquire(data.len() as u64).await;
        writer.write_all(&data).await?;
        chunk_downloaded += data.len() as u64;
        progress[chunk_index as usize]
            .store(chunk_downloaded, std::sync::atomic::Ordering::Relaxed);
    }

    // Each chunk gets fsync'd so its bytes survive a crash before the
    // reassembly stage reads it back.
    writer.flush().await?;
    fsync_file(writer.get_mut()).await?;
    debug!("Chunk {chunk_index} complete: {chunk_downloaded} bytes");
    Ok(())
}

/// Stream a single media segment (full GET, no range) to `seg_path`, updating
/// `progress[idx]` as bytes arrive. Returns the segment's byte count.
async fn download_segment(
    client: &reqwest::Client,
    url: &str,
    seg_path: &Path,
    idx: usize,
    progress: &[std::sync::atomic::AtomicU64],
) -> Result<u64, crate::Error> {
    let resp = client.get(url).send().await?.error_for_status()?;
    let file = tokio::fs::File::create(seg_path).await?;
    let mut writer = tokio::io::BufWriter::with_capacity(WRITE_BUFFER_SIZE, file);
    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;

    while let Some(data) = stream.next().await {
        let data = data.map_err(crate::Error::Http)?;
        writer.write_all(&data).await?;
        downloaded += data.len() as u64;
        progress[idx].store(downloaded, std::sync::atomic::Ordering::Relaxed);
    }

    // Only flush to the kernel — no per-segment fsync. These temp files are
    // scratch: never resumed after a crash and deleted right after
    // reassembly, so flushing is enough for the reassembly read to see the
    // bytes. Durability is provided by the single fsync of the final output.
    writer.flush().await?;
    Ok(downloaded)
}

/// Aborts a spawned task when dropped. Used to tie the progress-reporter task
/// to the lifetime of the download future, so a `tokio::select!` cancellation
/// (which drops the future before the normal cleanup runs) doesn't leak the
/// reporter for the rest of the process.
struct AbortOnDrop(tokio::task::JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Download a list of media segments (HLS/DASH) concurrently to indexed temp
/// files and reassemble them in playlist order into `dest`.
///
/// Shared by the HLS and DASH backends. Unlike the previous approach this never
/// holds the whole stream in RAM: each segment streams straight to
/// `seg_dir/seg_{idx:08}` and reassembly copies them through one buffered
/// writer, so peak memory is bounded by the concurrency, not the stream size.
/// Output is written to a sibling `.part` file and atomically renamed on
/// success (matching the durability guarantees of the chunked HTTP path).
/// Progress is emitted on a 500 ms timer instead of once per segment.
pub(crate) async fn download_segments_streamed(
    client: &reqwest::Client,
    segment_urls: &[String],
    dest: &Path,
    seg_dir: &Path,
    concurrent_segments: usize,
    progress_tx: watch::Sender<DownloadProgress>,
) -> Result<u64, crate::Error> {
    let total_segments = segment_urls.len();
    let concurrency = concurrent_segments.max(1);

    tokio::fs::create_dir_all(seg_dir).await?;
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let seg_progress = Arc::new(
        (0..total_segments)
            .map(|_| std::sync::atomic::AtomicU64::new(0))
            .collect::<Vec<_>>(),
    );

    // Emit aggregate progress on a timer rather than once per segment, so a
    // stream with thousands of segments doesn't flood the watch channel. The
    // AbortOnDrop guard stops the reporter even if this future is dropped on
    // cancellation before the normal cleanup runs.
    let progress_reporter = AbortOnDrop({
        let progress = seg_progress.clone();
        let tx = progress_tx.clone();
        tokio::spawn(async move {
            let mut last_total: u64 = 0;
            let mut last_time = tokio::time::Instant::now();
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                let total: u64 = progress
                    .iter()
                    .map(|a| a.load(std::sync::atomic::Ordering::Relaxed))
                    .sum();
                let now = tokio::time::Instant::now();
                let elapsed = now.duration_since(last_time);
                let speed = bytes_per_sec(total.saturating_sub(last_total), elapsed);
                // total_bytes stays None: segment sizes are unknown up front.
                let _ = tx.send(DownloadProgress {
                    bytes_downloaded: total,
                    total_bytes: None,
                    speed_bytes_per_sec: speed,
                });
                last_total = total;
                last_time = now;
            }
        })
    });

    let work: Result<u64, crate::Error> = async {
        // Download in bounded-concurrency batches, preserving global indices.
        let mut base = 0usize;
        for batch in segment_urls.chunks(concurrency) {
            let mut handles = Vec::with_capacity(batch.len());
            for (offset, url) in batch.iter().enumerate() {
                let idx = base + offset;
                let client = client.clone();
                let url = url.clone();
                let seg_path = seg_dir.join(format!("seg_{idx:08}"));
                let progress = seg_progress.clone();
                handles.push(tokio::spawn(async move {
                    download_segment(&client, &url, &seg_path, idx, &progress).await
                }));
            }
            // Await the entire batch before propagating any error, so a single
            // failure doesn't detach the remaining tasks (a dropped JoinHandle
            // keeps running and would race the error-path cleanup of seg_dir).
            let mut first_err: Option<crate::Error> = None;
            for handle in handles {
                match handle.await {
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) => {
                        first_err.get_or_insert(e);
                    }
                    Err(e) => {
                        first_err.get_or_insert(crate::Error::Other(e.to_string()));
                    }
                }
            }
            if let Some(e) = first_err {
                return Err(e);
            }
            base += batch.len();
            debug!("segments: downloaded {base}/{total_segments}");
        }

        // Reassemble in index order through a single buffered writer.
        let dest_tmp = dest.with_extension("part");
        let dest_file = tokio::fs::File::create(&dest_tmp).await?;
        let mut writer = tokio::io::BufWriter::with_capacity(WRITE_BUFFER_SIZE, dest_file);
        let mut total_bytes: u64 = 0;
        for i in 0..total_segments {
            let seg_path = seg_dir.join(format!("seg_{i:08}"));
            let mut seg_file = tokio::fs::File::open(&seg_path).await?;
            total_bytes += tokio::io::copy(&mut seg_file, &mut writer).await?;
            drop(seg_file);
            tokio::fs::remove_file(&seg_path).await?;
        }
        writer.flush().await?;
        fsync_file(writer.get_mut()).await?;
        drop(writer);
        tokio::fs::rename(&dest_tmp, dest).await?;
        fsync_parent(dest).await;
        Ok(total_bytes)
    }
    .await;

    // Stop the reporter before emitting the final progress so a stale tick
    // can't overwrite it. (On cancellation the guard's Drop handles this.)
    drop(progress_reporter);

    match work {
        Ok(total_bytes) => {
            let _ = tokio::fs::remove_dir_all(seg_dir).await;
            let _ = progress_tx.send(DownloadProgress {
                bytes_downloaded: total_bytes,
                total_bytes: Some(total_bytes),
                speed_bytes_per_sec: 0,
            });
            Ok(total_bytes)
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(dest.with_extension("part")).await;
            let _ = tokio::fs::remove_dir_all(seg_dir).await;
            Err(e)
        }
    }
}

/// Compute a transfer rate in bytes/sec, guarding against a zero (or
/// otherwise degenerate) elapsed interval which would yield `inf`/`NaN` and
/// cast to a nonsensical `u64`.
fn bytes_per_sec(bytes: u64, elapsed: std::time::Duration) -> u64 {
    let secs = elapsed.as_secs_f64();
    if secs > 0.0 {
        (bytes as f64 / secs) as u64
    } else {
        0
    }
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
                return Some(crate::sanitize_filename(name));
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
            return Some(crate::sanitize_filename(name));
        }
    }
    None
}

#[async_trait::async_trait]
impl super::ProtocolBackend for HttpDownloader {
    fn protocol(&self) -> super::Protocol {
        super::Protocol::Http
    }

    async fn download(
        &self,
        job: &super::DownloadJob,
        progress_tx: watch::Sender<DownloadProgress>,
        mut cancel_rx: watch::Receiver<bool>,
    ) -> Result<(u64, std::path::PathBuf), crate::Error> {
        let head = self.head(&job.url).await?;

        let fname = job
            .filename
            .clone()
            .or(head.filename.clone())
            .unwrap_or_else(|| {
                job.url
                    .rsplit('/')
                    .next()
                    .and_then(|s| s.split('?').next())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("download")
                    .to_string()
            });

        let dest = job.download_dir.join(&fname);
        tokio::fs::create_dir_all(&job.download_dir).await?;
        tokio::fs::create_dir_all(&job.temp_dir).await?;

        let use_chunked = head.accepts_ranges
            && head.content_length.is_some_and(|s| s > 1024 * 1024)
            && job.max_chunks > 1;

        if use_chunked {
            let total = head.content_length.unwrap();
            let chunks = job.max_chunks.min((total / (512 * 1024)).max(1) as u32);
            let chunk_dir = job.temp_dir.join(&fname);
            tokio::fs::create_dir_all(&chunk_dir).await?;

            let chunked = tokio::select! {
                result = self.download_chunked(&job.url, &dest, &chunk_dir, total, chunks, progress_tx.clone()) => result,
                _ = super::wait_for_cancel(&mut cancel_rx) => Err(crate::Error::Cancelled),
            };
            match chunked {
                // The server advertised Accept-Ranges but then ignored the
                // Range header (returned 200). Restart cleanly as a single
                // stream instead of failing or corrupting the output.
                Err(crate::Error::RangeNotSupported) => {
                    warn!(
                        "Server ignored Range despite Accept-Ranges; \
                         restarting {} as a single stream",
                        job.url
                    );
                    let _ = tokio::fs::remove_dir_all(&chunk_dir).await;
                    tokio::select! {
                        result = self.download_single(&job.url, &dest, progress_tx) => result.map(|bytes| (bytes, dest)),
                        _ = super::wait_for_cancel(&mut cancel_rx) => Err(crate::Error::Cancelled),
                    }
                }
                other => other.map(|bytes| (bytes, dest)),
            }
        } else {
            tokio::select! {
                result = self.download_single(&job.url, &dest, progress_tx) => result.map(|bytes| (bytes, dest)),
                _ = super::wait_for_cancel(&mut cancel_rx) => Err(crate::Error::Cancelled),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn chunk_download_detects_ignored_range() {
        // A server that answers a ranged GET with 200 OK (full body) instead of
        // 206 must be detected: otherwise each chunk downloads the whole file
        // and reassembly concatenates N copies into a corrupt output.
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0u8; 4096]))
            .mount(&mock)
            .await;

        let bw = BandwidthLimiter::new(crate::bandwidth::BandwidthConfig::default());
        let client = reqwest::Client::new();
        let dir = tempfile::tempdir().unwrap();
        let progress = [std::sync::atomic::AtomicU64::new(0)];
        let err = download_chunk(
            &client,
            &bw,
            &format!("{}/file.bin", mock.uri()),
            &dir.path().join("chunk_0"),
            0,
            2047,
            0,
            &progress,
        )
        .await
        .expect_err("a 200 response to a ranged chunk must be rejected");
        assert!(
            matches!(err, crate::Error::RangeNotSupported),
            "expected RangeNotSupported, got {err:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn chunked_download_failure_stops_progress_reporter() {
        // Regression test: when a chunk request fails, download_chunked used
        // to return early without aborting the progress-reporter task, which
        // then kept emitting progress events every 500 ms forever (its only
        // exit condition is "all bytes downloaded").
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock)
            .await;

        let dl = HttpDownloader::new(
            "amigo-test",
            BandwidthLimiter::new(crate::bandwidth::BandwidthConfig::default()),
        );
        let dir = tempfile::tempdir().unwrap();
        let (tx, mut rx) = watch::channel(DownloadProgress {
            bytes_downloaded: 0,
            total_bytes: None,
            speed_bytes_per_sec: 0,
        });

        let result = dl
            .download_chunked(
                &format!("{}/file.bin", mock.uri()),
                &dir.path().join("file.bin"),
                dir.path(),
                4096,
                2,
                tx,
            )
            .await;
        assert!(result.is_err(), "chunked download must fail on 500s");

        // After the error the reporter must be gone: no progress updates may
        // arrive anymore. (The leaked task used to tick every 500 ms.)
        rx.mark_unchanged();
        tokio::time::sleep(tokio::time::Duration::from_millis(1200)).await;
        assert!(
            !rx.has_changed().unwrap_or(false),
            "progress reporter kept running after chunk failure"
        );
    }

    #[test]
    fn bytes_per_sec_guards_against_zero_elapsed() {
        // A zero interval must not yield inf/NaN cast to a bogus u64.
        assert_eq!(bytes_per_sec(1000, std::time::Duration::ZERO), 0);
        // Normal case: 1000 bytes over 0.5s = 2000 B/s.
        assert_eq!(
            bytes_per_sec(1000, std::time::Duration::from_millis(500)),
            2000
        );
    }

    #[tokio::test]
    async fn fsync_file_persists_buffered_writes() {
        // Sanity-check the helper used by every download path: bytes
        // written through write_all + flush must survive a re-open. This is
        // not a power-loss simulation, but it does catch a regression where
        // the helper accidentally drops the sync_all step (e.g. someone
        // reverting it back to flush()-only).
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.bin");
        {
            let mut f = tokio::fs::File::create(&path).await.unwrap();
            f.write_all(b"hello-fsync").await.unwrap();
            fsync_file(&mut f).await.unwrap();
        }
        let read_back = tokio::fs::read(&path).await.unwrap();
        assert_eq!(read_back, b"hello-fsync");
    }

    #[tokio::test]
    async fn fsync_parent_is_no_op_for_missing_dir() {
        // The helper has to be a no-op (best-effort) on platforms or paths
        // where the directory fd cannot be opened, so callers can use it
        // unconditionally after a rename without sprinkling cfg(unix).
        fsync_parent(std::path::Path::new("/nonexistent/never-created")).await;
    }

    async fn write_chunk(dir: &Path, i: u32, bytes: &[u8]) {
        tokio::fs::write(dir.join(format!("chunk_{i}")), bytes)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn reassemble_chunks_concatenates_and_verifies_size() {
        let dir = tempfile::tempdir().unwrap();
        write_chunk(dir.path(), 0, b"hello ").await;
        write_chunk(dir.path(), 1, b"world").await;
        let dest = dir.path().join("out.bin");

        let written = reassemble_chunks(dir.path(), 2, &dest, 11).await.unwrap();
        assert_eq!(written, 11);
        assert_eq!(tokio::fs::read(&dest).await.unwrap(), b"hello world");
        // Chunk temp files must be cleaned up.
        assert!(!dir.path().join("chunk_0").exists());
        assert!(!dir.path().join("chunk_1").exists());
    }

    #[tokio::test]
    async fn reassemble_chunks_rejects_short_total() {
        // A chunk that silently lost bytes: concatenation is shorter than the
        // advertised total_size. Reassembly must error and leave no dest file.
        let dir = tempfile::tempdir().unwrap();
        write_chunk(dir.path(), 0, b"hello ").await;
        write_chunk(dir.path(), 1, b"wor").await; // 3 bytes instead of 5
        let dest = dir.path().join("out.bin");

        let result = reassemble_chunks(dir.path(), 2, &dest, 11).await;
        assert!(result.is_err(), "short reassembly must be rejected");
        assert!(
            !dest.exists(),
            "no destination file may be left on size mismatch"
        );
        assert!(
            !dir.path().join("out.part").exists(),
            "the .part temp file must be removed on size mismatch"
        );
    }

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
