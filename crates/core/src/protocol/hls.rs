//! HLS (HTTP Live Streaming) downloader.
//!
//! Parses m3u8 manifests and downloads segments in parallel, then concatenates
//! them into a single output file.

use std::path::Path;

use tokio::sync::watch;
use tracing::info;

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
        seg_dir: &Path,
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
                        self.download_segments(&variant_url, &pl, dest, seg_dir, progress_tx)
                            .await
                    }
                    _ => Err(crate::Error::Other(
                        "Expected media playlist but got master".into(),
                    )),
                }
            }
            m3u8_rs::Playlist::MediaPlaylist(media) => {
                self.download_segments(manifest_url, &media, dest, seg_dir, progress_tx)
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
        seg_dir: &Path,
        progress_tx: watch::Sender<DownloadProgress>,
    ) -> Result<u64, crate::Error> {
        let segment_urls: Vec<String> = playlist
            .segments
            .iter()
            .map(|seg| resolve_url(playlist_url, &seg.uri))
            .collect();

        info!(
            "HLS: downloading {} segments ({} concurrent)",
            segment_urls.len(),
            self.concurrent_segments
        );

        let total_bytes = super::http::download_segments_streamed(
            &self.client,
            &segment_urls,
            dest,
            seg_dir,
            self.concurrent_segments,
            progress_tx,
        )
        .await?;

        info!("HLS: wrote {} bytes to {}", total_bytes, dest.display());
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
        mut cancel_rx: watch::Receiver<bool>,
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
        // Per-download scratch space for the segment temp files.
        let seg_dir = job.temp_dir.join(format!("hls-{}", job.download_id));

        tokio::select! {
            result = self.download(&job.url, &dest, &seg_dir, progress_tx) => result.map(|bytes| (bytes, dest)),
            _ = super::wait_for_cancel(&mut cancel_rx) => Err(crate::Error::Cancelled),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn reassembles_segments_in_playlist_order() {
        let server = MockServer::start().await;

        // A media playlist referencing three relative segments, intentionally
        // served so that concurrent download could finish out of order.
        let playlist = "#EXTM3U\n\
             #EXT-X-VERSION:3\n\
             #EXT-X-TARGETDURATION:4\n\
             #EXTINF:4.0,\nseg0.ts\n\
             #EXTINF:4.0,\nseg1.ts\n\
             #EXTINF:4.0,\nseg2.ts\n\
             #EXT-X-ENDLIST\n";
        Mock::given(method("GET"))
            .and(path("/playlist.m3u8"))
            .respond_with(ResponseTemplate::new(200).set_body_string(playlist))
            .mount(&server)
            .await;

        // Distinct, different-length bodies so a wrong order is unambiguous.
        let bodies: [&[u8]; 3] = [b"AAAA", b"BB", b"CCCCCC"];
        for (i, body) in bodies.iter().enumerate() {
            Mock::given(method("GET"))
                .and(path(format!("/seg{i}.ts")))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(body.to_vec()))
                .mount(&server)
                .await;
        }

        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("out.ts");
        let seg_dir = tmp.path().join("segments");
        let (tx, _rx) = watch::channel(DownloadProgress {
            bytes_downloaded: 0,
            total_bytes: None,
            speed_bytes_per_sec: 0,
        });

        let downloader = HlsDownloader::new("test-agent", 3);
        let manifest_url = format!("{}/playlist.m3u8", server.uri());
        let written = downloader
            .download(&manifest_url, &dest, &seg_dir, tx)
            .await
            .expect("HLS download should succeed");

        let expected: Vec<u8> = bodies.concat();
        assert_eq!(written, expected.len() as u64);
        let on_disk = tokio::fs::read(&dest).await.unwrap();
        assert_eq!(on_disk, expected, "segments must be concatenated in order");
        // Scratch dir must be cleaned up on success.
        assert!(!seg_dir.exists(), "segment temp dir should be removed");
    }
}
