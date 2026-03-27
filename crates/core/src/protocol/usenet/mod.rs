//! Usenet/NNTP download backend.
//!
//! Pipeline: NZB parse → NNTP download articles → yEnc decode → reassemble → PAR2 verify/repair

pub mod nntp;
pub mod nzb;
pub mod yenc;

use std::path::{Path, PathBuf};

use tokio::io::AsyncWriteExt;
use tracing::{debug, error, info, warn};

use self::nntp::{NntpConnectionPool, NntpServerConfig};
use self::nzb::{NzbFile, NzbSegment};
use self::yenc::decode_yenc;

/// Usenet download configuration.
#[derive(Debug, Clone)]
pub struct UsenetConfig {
    pub servers: Vec<NntpServerConfig>,
    pub par2_repair: bool,
    pub auto_unrar: bool,
    pub delete_archives_after_extract: bool,
}

/// Result of downloading a single NZB file entry.
#[derive(Debug)]
pub struct FileDownloadResult {
    pub filename: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub segments_ok: u32,
    pub segments_failed: u32,
}

pub struct UsenetDownloader {
    pools: Vec<NntpConnectionPool>,
}

impl UsenetDownloader {
    pub fn new(config: UsenetConfig) -> Self {
        let mut pools: Vec<NntpConnectionPool> = config
            .servers
            .iter()
            .map(|s| NntpConnectionPool::new(s.clone()))
            .collect();

        // Sort by priority (lower = higher priority)
        pools.sort_by_key(|p| p.priority());

        Self { pools }
    }

    /// Download all files from an NZB.
    pub async fn download_nzb(
        &self,
        nzb_data: &str,
        output_dir: &Path,
    ) -> Result<Vec<FileDownloadResult>, crate::Error> {
        let nzb = nzb::parse_nzb(nzb_data)?;
        info!("NZB contains {} files", nzb.files.len());

        tokio::fs::create_dir_all(output_dir).await?;

        let mut results = Vec::new();

        for file in &nzb.files {
            let filename = file.filename();
            info!("Downloading: {filename} ({} segments)", file.segments.len());

            match self.download_file(file, output_dir).await {
                Ok(result) => {
                    info!(
                        "Completed: {filename} ({} bytes, {}/{} segments)",
                        result.bytes,
                        result.segments_ok,
                        result.segments_ok + result.segments_failed
                    );
                    results.push(result);
                }
                Err(e) => {
                    error!("Failed to download {filename}: {e}");
                    results.push(FileDownloadResult {
                        filename,
                        path: output_dir.to_path_buf(),
                        bytes: 0,
                        segments_ok: 0,
                        segments_failed: file.segments.len() as u32,
                    });
                }
            }
        }

        Ok(results)
    }

    /// Download a single file from its NZB segments.
    async fn download_file(
        &self,
        file: &NzbFile,
        output_dir: &Path,
    ) -> Result<FileDownloadResult, crate::Error> {
        let filename = file.filename();
        let output_path = output_dir.join(&filename);
        let group = file.groups.first().cloned().unwrap_or_default();

        let mut all_data: Vec<(u32, Vec<u8>)> = Vec::new();
        let mut segments_ok: u32 = 0;
        let mut segments_failed: u32 = 0;

        for segment in &file.segments {
            match self.download_segment(&group, segment).await {
                Ok(data) => {
                    all_data.push((segment.number, data));
                    segments_ok += 1;
                }
                Err(e) => {
                    warn!(
                        "Segment {} of {filename} failed: {e}",
                        segment.number
                    );
                    segments_failed += 1;
                }
            }
        }

        // Sort segments by number and write to file
        all_data.sort_by_key(|(num, _)| *num);

        let mut out_file = tokio::fs::File::create(&output_path).await?;
        let mut total_bytes: u64 = 0;

        for (_, data) in &all_data {
            out_file.write_all(data).await?;
            total_bytes += data.len() as u64;
        }
        out_file.flush().await?;

        Ok(FileDownloadResult {
            filename,
            path: output_path,
            bytes: total_bytes,
            segments_ok,
            segments_failed,
        })
    }

    /// Download and decode a single segment, trying servers in priority order.
    async fn download_segment(
        &self,
        group: &str,
        segment: &NzbSegment,
    ) -> Result<Vec<u8>, crate::Error> {
        let mut last_error = None;

        for pool in &self.pools {
            match self.try_segment_from_pool(pool, group, segment).await {
                Ok(data) => return Ok(data),
                Err(e) => {
                    debug!(
                        "Segment {} failed on {}: {e}",
                        segment.message_id,
                        pool.server_name()
                    );
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| crate::Error::Other("No NNTP servers configured".into())))
    }

    async fn try_segment_from_pool(
        &self,
        pool: &NntpConnectionPool,
        group: &str,
        segment: &NzbSegment,
    ) -> Result<Vec<u8>, crate::Error> {
        let mut conn = pool.acquire().await?;

        // Select group
        let resp = conn.group(group).await?;
        if resp.code != 211 {
            pool.release(conn).await;
            return Err(crate::Error::Other(format!(
                "GROUP failed: {} {}",
                resp.code, resp.message
            )));
        }

        // Download article body
        let body = conn.body(&segment.message_id).await;
        pool.release(conn).await;
        let body = body?;

        // Decode yEnc
        let decoded = decode_yenc(&body)?;

        // Verify CRC if available
        if let Some(expected_crc) = decoded.part_crc32.or(decoded.crc32) {
            if !yenc::verify_crc32(&decoded.data, expected_crc) {
                return Err(crate::Error::Other(format!(
                    "CRC32 mismatch for segment {}",
                    segment.message_id
                )));
            }
        }

        Ok(decoded.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usenet_config() {
        let config = UsenetConfig {
            servers: vec![NntpServerConfig {
                name: "primary".into(),
                host: "news.example.com".into(),
                port: 563,
                ssl: true,
                username: "user".into(),
                password: "pass".into(),
                connections: 20,
                priority: 0,
            }],
            par2_repair: true,
            auto_unrar: true,
            delete_archives_after_extract: true,
        };

        let downloader = UsenetDownloader::new(config);
        assert_eq!(downloader.pools.len(), 1);
    }
}
