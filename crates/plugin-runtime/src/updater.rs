//! Plugin update orchestration.
//!
//! Coordinates registry checks, downloads, and hot-reload of plugins.

use std::sync::Arc;

use tracing::{info, warn};

use crate::loader::PluginLoader;
use crate::registry::{self, PluginUpdateInfo, RegistryConfig, RegistryPlugin};
use crate::types::PluginMeta;

/// Orchestrates plugin updates.
pub struct PluginUpdater {
    config: RegistryConfig,
    client: reqwest::Client,
    loader: Arc<PluginLoader>,
}

impl PluginUpdater {
    pub fn new(config: RegistryConfig, client: reqwest::Client, loader: Arc<PluginLoader>) -> Self {
        Self {
            config,
            client,
            loader,
        }
    }

    /// Check for available plugin updates.
    pub async fn check_updates(&self) -> Result<Vec<PluginUpdateInfo>, crate::Error> {
        let index = registry::fetch_index(&self.client, &self.config).await?;
        let installed = self.loader.list_plugins().await;
        Ok(registry::check_plugin_updates(&index, &installed))
    }

    /// Install a new plugin from the registry.
    pub async fn install_plugin(&self, plugin_id: &str) -> Result<PluginMeta, crate::Error> {
        let index = registry::fetch_index(&self.client, &self.config).await?;
        let registry_plugin = index
            .plugins
            .iter()
            .find(|p| p.id == plugin_id)
            .ok_or_else(|| crate::Error::NotFound(format!("Plugin {plugin_id} not in registry")))?;

        let hosters_dir = self.loader.plugin_dir().join("hosters");
        registry::download_plugin(&self.client, registry_plugin, &hosters_dir).await?;

        // Re-discover to pick up the new plugin
        self.loader.discover().await?;

        self.loader.get_plugin_meta(plugin_id).await.ok_or_else(|| {
            crate::Error::Other(format!("Plugin {plugin_id} installed but failed to load"))
        })
    }

    /// Update an existing plugin to the latest version.
    pub async fn update_plugin(&self, plugin_id: &str) -> Result<PluginMeta, crate::Error> {
        let index = registry::fetch_index(&self.client, &self.config).await?;
        let registry_plugin = index
            .plugins
            .iter()
            .find(|p| p.id == plugin_id)
            .ok_or_else(|| crate::Error::NotFound(format!("Plugin {plugin_id} not in registry")))?;

        // Download to hosters dir (overwrites existing via atomic rename)
        let hosters_dir = self.loader.plugin_dir().join("hosters");
        registry::download_plugin(&self.client, registry_plugin, &hosters_dir).await?;

        // Hot-reload the plugin
        let meta = self.loader.reload(plugin_id).await?;
        info!("Plugin {} updated to v{}", plugin_id, meta.version);

        Ok(meta)
    }

    /// Update all plugins that have newer versions available.
    pub async fn update_all_plugins(&self) -> Result<Vec<PluginMeta>, crate::Error> {
        let updates = self.check_updates().await?;
        let mut updated = Vec::new();

        for update_info in &updates {
            if update_info.is_new {
                continue; // Skip new plugins — only update existing
            }

            match self.update_plugin(&update_info.plugin_id).await {
                Ok(meta) => updated.push(meta),
                Err(e) => {
                    warn!("Failed to update plugin {}: {e}", update_info.plugin_id);
                }
            }
        }

        Ok(updated)
    }

    /// List all available plugins from the registry (marketplace).
    pub async fn list_available(&self) -> Result<Vec<RegistryPlugin>, crate::Error> {
        let index = registry::fetch_index(&self.client, &self.config).await?;
        Ok(index.plugins)
    }
}
