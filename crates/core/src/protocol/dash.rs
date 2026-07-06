//! DASH (Dynamic Adaptive Streaming over HTTP) downloader.
//!
//! Parses MPD manifests and downloads segments in parallel, then reassembles
//! them (init segment + media segments) into a single output file.

use std::path::Path;

use tokio::sync::watch;
use tracing::info;

use super::http::DownloadProgress;

/// DASH downloader.
pub struct DashDownloader {
    client: reqwest::Client,
    concurrent_segments: usize,
}

impl DashDownloader {
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

    /// Download a DASH stream from an MPD manifest URL.
    pub async fn download(
        &self,
        mpd_url: &str,
        dest: &Path,
        seg_dir: &Path,
        progress_tx: watch::Sender<DownloadProgress>,
    ) -> Result<u64, crate::Error> {
        info!("Downloading DASH stream: {mpd_url}");

        let mpd_body = self.client.get(mpd_url).send().await?.text().await?;

        let mpd: dash_mpd::MPD = dash_mpd::parse(&mpd_body)
            .map_err(|e| crate::Error::Other(format!("Failed to parse MPD: {e}")))?;

        // Select the best representation from the first period
        let period = mpd
            .periods
            .first()
            .ok_or_else(|| crate::Error::Other("No periods in MPD".into()))?;

        // Find best video adaptation set + representation
        let mut best_repr = None;
        let mut best_bandwidth = 0u64;
        let mut best_adaptation = None;

        for adaptation in &period.adaptations {
            let content_type = adaptation
                .contentType
                .as_deref()
                .or(adaptation.mimeType.as_deref())
                .unwrap_or("");

            let is_video = content_type.contains("video")
                || adaptation
                    .representations
                    .iter()
                    .any(|r| r.mimeType.as_deref().unwrap_or("").contains("video"));

            if !is_video {
                continue;
            }

            for repr in &adaptation.representations {
                let bw = repr.bandwidth.unwrap_or(0);
                if bw > best_bandwidth {
                    best_bandwidth = bw;
                    best_repr = Some(repr);
                    best_adaptation = Some(adaptation);
                }
            }
        }

        let repr = best_repr
            .ok_or_else(|| crate::Error::Other("No video representation found in MPD".into()))?;
        let adaptation =
            best_adaptation.ok_or_else(|| crate::Error::Other("No adaptation set".into()))?;

        info!(
            "Selected DASH representation: bandwidth={best_bandwidth}, id={:?}",
            repr.id
        );

        // Build segment URLs
        let segment_urls = build_segment_urls(mpd_url, period, adaptation, repr)?;

        if segment_urls.is_empty() {
            return Err(crate::Error::Other(
                "No segment URLs could be generated from MPD".into(),
            ));
        }

        info!(
            "DASH: downloading {} segments ({} concurrent)",
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

        info!("DASH: wrote {} bytes to {}", total_bytes, dest.display());
        Ok(total_bytes)
    }
}

/// Upper bound on the number of media segments a single representation may
/// expand to. A malformed/malicious MPD (tiny segment duration, or a huge
/// SegmentTimeline `r` repeat count) could otherwise allocate a multi-billion
/// element Vec and exhaust memory. 100k comfortably covers a multi-hour stream
/// at typical 2–6 s segment durations.
const MAX_DASH_SEGMENTS: usize = 100_000;

/// Build segment URLs from MPD structure.
fn build_segment_urls(
    mpd_url: &str,
    period: &dash_mpd::Period,
    adaptation: &dash_mpd::AdaptationSet,
    repr: &dash_mpd::Representation,
) -> Result<Vec<String>, crate::Error> {
    let mut urls = Vec::new();

    // Check for SegmentTemplate
    let seg_template = repr
        .SegmentTemplate
        .as_ref()
        .or(adaptation.SegmentTemplate.as_ref())
        .or(period.SegmentTemplate.as_ref());

    if let Some(template) = seg_template {
        // Init segment
        if let Some(init) = &template.initialization {
            let init_url = expand_template(init, repr, 0);
            urls.push(resolve_url(mpd_url, &init_url));
        }

        // Media segments
        if let Some(media_template) = &template.media {
            if let Some(timeline) = &template.SegmentTimeline {
                // SegmentTimeline mode
                let mut time: u64 = 0;
                let mut number: u64 = template.startNumber.unwrap_or(1);

                for s in &timeline.segments {
                    let t = s.t.unwrap_or(time);
                    time = t;
                    // `r` may be negative ("repeat until end of period"), which
                    // we do not expand; clamp to 0 so it produces a single
                    // segment rather than a garbage/negative range.
                    let repeat = s.r.unwrap_or(0).max(0);

                    for _ in 0..=repeat {
                        if urls.len() >= MAX_DASH_SEGMENTS {
                            return Err(crate::Error::Other(format!(
                                "DASH manifest expands to more than {MAX_DASH_SEGMENTS} \
                                 segments — refusing (malformed or malicious manifest)"
                            )));
                        }
                        let url = expand_template_with_time(media_template, repr, number, time);
                        urls.push(resolve_url(mpd_url, &url));
                        time += s.d;
                        number += 1;
                    }
                }
            } else if let Some(duration) = template.duration {
                // Duration-based segments
                let timescale = template.timescale.unwrap_or(1).max(1);
                let period_duration = period
                    .duration
                    .as_ref()
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(3600.0);

                let segment_duration = duration / timescale as f64;
                if segment_duration > 0.0 {
                    let num_segments = (period_duration / segment_duration).ceil() as u64;
                    if num_segments > MAX_DASH_SEGMENTS as u64 {
                        return Err(crate::Error::Other(format!(
                            "DASH manifest declares {num_segments} segments \
                             (> {MAX_DASH_SEGMENTS}) — refusing (malformed or malicious manifest)"
                        )));
                    }
                    let start_number = template.startNumber.unwrap_or(1);

                    for i in 0..num_segments {
                        let number = start_number + i;
                        let url = expand_template(media_template, repr, number);
                        urls.push(resolve_url(mpd_url, &url));
                    }
                }
            }
        }
    }

    // Check for SegmentList
    if urls.is_empty() {
        let seg_list = repr
            .SegmentList
            .as_ref()
            .or(adaptation.SegmentList.as_ref());

        if let Some(list) = seg_list {
            if let Some(init) = &list.Initialization
                && let Some(source) = &init.sourceURL
            {
                urls.push(resolve_url(mpd_url, source));
            }
            for seg_url in &list.segment_urls {
                if let Some(media) = &seg_url.media {
                    urls.push(resolve_url(mpd_url, media));
                }
            }
        }
    }

    // Check for BaseURL (single segment)
    if urls.is_empty()
        && let Some(base_url) = repr.BaseURL.first()
    {
        urls.push(resolve_url(mpd_url, &base_url.base));
    }

    Ok(urls)
}

/// Expand a SegmentTemplate URL pattern, replacing $RepresentationID$, $Number$, etc.
fn expand_template(template: &str, repr: &dash_mpd::Representation, number: u64) -> String {
    let mut result = template.to_string();
    if let Some(id) = &repr.id {
        result = result.replace("$RepresentationID$", id);
    }
    result = result.replace("$Number$", &number.to_string());

    // Handle $Number%0Xd$ patterns (zero-padded numbers)
    static DASH_NUMBER_RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"\$Number%(\d+)d\$").unwrap());
    result = DASH_NUMBER_RE
        .replace_all(&result, |caps: &regex::Captures| {
            let width: usize = caps[1].parse().unwrap_or(1);
            format!("{:0>width$}", number, width = width)
        })
        .to_string();

    if let Some(bw) = repr.bandwidth {
        result = result.replace("$Bandwidth$", &bw.to_string());
    }

    result
}

/// Expand a SegmentTemplate URL pattern with time-based replacement.
fn expand_template_with_time(
    template: &str,
    repr: &dash_mpd::Representation,
    number: u64,
    time: u64,
) -> String {
    let mut result = expand_template(template, repr, number);
    result = result.replace("$Time$", &time.to_string());
    result
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

    if let Some(pos) = base.rfind('/') {
        format!("{}/{relative}", &base[..pos])
    } else {
        relative.to_string()
    }
}

/// Check if a URL looks like a DASH manifest.
pub fn is_dash_url(url: &str) -> bool {
    let path = url.split('?').next().unwrap_or(url);
    path.ends_with(".mpd")
}

#[async_trait::async_trait]
impl super::ProtocolBackend for DashDownloader {
    fn protocol(&self) -> super::Protocol {
        super::Protocol::Dash
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
                .replace(".mpd", ".mp4")
        });
        let dest = job.download_dir.join(&fname);
        tokio::fs::create_dir_all(&job.download_dir).await?;
        // Per-download scratch space for the segment temp files.
        let seg_dir = job.temp_dir.join(format!("dash-{}", job.download_id));

        tokio::select! {
            result = self.download(&job.url, &dest, &seg_dir, progress_tx) => result.map(|bytes| (bytes, dest)),
            _ = super::wait_for_cancel(&mut cancel_rx) => Err(crate::Error::Cancelled),
        }
    }
}
