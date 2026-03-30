//! URL resolvers that bridge plugins and extractors to the core download pipeline.

use std::collections::HashMap;
use std::sync::Arc;

use amigo_core::protocol::{Protocol, ResolvedDownload, UrlResolver};
use amigo_extractors::{Extractor, StreamProtocol};
use amigo_plugin_runtime::loader::PluginLoader;
use amigo_plugin_runtime::types::DownloadProtocol;

/// Wraps `PluginLoader` to implement the core `UrlResolver` trait.
pub struct PluginUrlResolver {
    loader: Arc<PluginLoader>,
}

impl PluginUrlResolver {
    pub fn new(loader: Arc<PluginLoader>) -> Self {
        Self { loader }
    }
}

#[async_trait::async_trait]
impl UrlResolver for PluginUrlResolver {
    async fn resolve(&self, url: &str) -> Option<ResolvedDownload> {
        // Check if any plugin matches this URL
        let plugin_meta = self.loader.match_url(url).await?;

        tracing::debug!("Plugin '{}' matched URL: {url}", plugin_meta.name);

        // Call the plugin's resolve() function
        let package = match self.loader.resolve(&plugin_meta.id, url).await {
            Ok(pkg) => pkg,
            Err(e) => {
                tracing::warn!(
                    "Plugin '{}' failed to resolve {url}: {e}",
                    plugin_meta.name
                );
                return None;
            }
        };

        // Take the first download from the package
        let info = package.downloads.into_iter().next()?;

        let protocol = match info.protocol {
            DownloadProtocol::Hls => Protocol::Hls,
            DownloadProtocol::Dash => Protocol::Dash,
            DownloadProtocol::Http => Protocol::Http,
        };

        Some(ResolvedDownload {
            url: info.url,
            filename: info.filename,
            filesize: info.filesize,
            protocol,
            headers: info.headers.unwrap_or_default(),
        })
    }
}

/// Wraps native Rust extractors (YouTube, Generic) as a `UrlResolver`.
pub struct ExtractorUrlResolver {
    extractors: Vec<Arc<dyn Extractor>>,
    client: reqwest::Client,
}

impl ExtractorUrlResolver {
    pub fn new(extractors: Vec<Arc<dyn Extractor>>, client: reqwest::Client) -> Self {
        Self { extractors, client }
    }
}

#[async_trait::async_trait]
impl UrlResolver for ExtractorUrlResolver {
    async fn resolve(&self, url: &str) -> Option<ResolvedDownload> {
        for extractor in &self.extractors {
            if !extractor.supports_url(url) {
                continue;
            }

            tracing::debug!("Extractor '{}' matched URL: {url}", extractor.name());

            let media = match extractor.extract(&self.client, url).await {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!(
                        "Extractor '{}' failed for {url}: {e}",
                        extractor.name()
                    );
                    continue;
                }
            };

            // Pick the best stream: prefer video+audio, highest resolution
            let best = media
                .streams
                .iter()
                .filter(|s| s.has_video && s.has_audio)
                .max_by_key(|s| s.height)
                .or_else(|| media.streams.first())?;

            let protocol = match best.protocol {
                StreamProtocol::Hls => Protocol::Hls,
                StreamProtocol::Dash => Protocol::Dash,
                StreamProtocol::Http => Protocol::Http,
            };

            return Some(ResolvedDownload {
                url: best.url.clone(),
                filename: Some(format!("{}.mp4", media.title)),
                filesize: best.filesize,
                protocol,
                headers: HashMap::new(),
            });
        }

        None
    }
}
