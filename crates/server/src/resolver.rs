//! URL resolver that delegates to the plugin system.

use std::sync::Arc;

use amigo_core::protocol::{Protocol, ResolvedDownload, UrlResolver};
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
