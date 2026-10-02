//! Plugin update orchestration.
//!
//! Coordinates registry checks, downloads, and hot-reload of plugins.

use std::sync::Arc;

use tracing::{info, warn};

use crate::loader::PluginLoader;
use crate::permissions;
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
    ///
    /// `approved` records that the user was shown the plugin's declared
    /// domains (or its unscoped status) and confirmed them. Without it the
    /// install is refused with [`crate::Error::PermissionApprovalRequired`],
    /// whose message lists what would be granted.
    pub async fn install_plugin(
        &self,
        plugin_id: &str,
        approved: bool,
    ) -> Result<PluginMeta, crate::Error> {
        let index = registry::fetch_index(&self.client, &self.config).await?;
        let registry_plugin = index
            .plugins
            .iter()
            .find(|p| p.id == plugin_id)
            .ok_or_else(|| crate::Error::NotFound(format!("Plugin {plugin_id} not in registry")))?;

        if !approved {
            return Err(crate::Error::PermissionApprovalRequired(format!(
                "installing {plugin_id} grants network access to {}",
                describe_domains(registry_plugin.declared_domains())
            )));
        }

        let hosters_dir = self.loader.plugin_dir().to_path_buf();
        let path = registry::download_plugin(&self.client, registry_plugin, &hosters_dir).await?;

        let meta = self.loader.load_plugin(&path).await?;
        self.enforce_approved_domains(&meta, registry_plugin.declared_domains())
            .await?;
        Ok(meta)
    }

    /// Update an existing plugin to the latest version.
    ///
    /// An update whose declared domains are wider than the installed
    /// version's is refused with [`crate::Error::PermissionApprovalRequired`]
    /// unless `approved` is set — it is never applied silently.
    pub async fn update_plugin(
        &self,
        plugin_id: &str,
        approved: bool,
    ) -> Result<PluginMeta, crate::Error> {
        let index = registry::fetch_index(&self.client, &self.config).await?;
        let registry_plugin = index
            .plugins
            .iter()
            .find(|p| p.id == plugin_id)
            .ok_or_else(|| crate::Error::NotFound(format!("Plugin {plugin_id} not in registry")))?;

        let installed = self
            .loader
            .get_plugin_meta(plugin_id)
            .await
            .ok_or_else(|| crate::Error::NotFound(plugin_id.to_string()))?;
        let installed_domains = installed.permissions.domains.clone();
        if let Some(added) = permissions::widened_domains(
            installed_domains.as_deref(),
            registry_plugin.declared_domains(),
        ) && !approved
        {
            return Err(crate::Error::PermissionApprovalRequired(format!(
                "updating {plugin_id} to v{} adds network access to {}",
                registry_plugin.version,
                added.join(", ")
            )));
        }

        // Download to hosters dir (overwrites existing via atomic rename)
        let hosters_dir = self.loader.plugin_dir().to_path_buf();
        registry::download_plugin(&self.client, registry_plugin, &hosters_dir).await?;

        // Hot-reload the plugin
        let meta = self.loader.reload(plugin_id).await?;
        let approved_domains = if approved {
            registry_plugin.declared_domains()
        } else {
            installed_domains.as_deref()
        };
        self.enforce_approved_domains(&meta, approved_domains)
            .await?;
        info!("Plugin {} updated to v{}", plugin_id, meta.version);

        Ok(meta)
    }

    /// Update all plugins that have newer versions available. Updates that
    /// would widen a plugin's network access are skipped (they need an
    /// explicit, approved [`Self::update_plugin`]).
    pub async fn update_all_plugins(&self) -> Result<Vec<PluginMeta>, crate::Error> {
        let updates = self.check_updates().await?;
        let mut updated = Vec::new();

        for update_info in &updates {
            if update_info.is_new {
                continue; // Skip new plugins — only update existing
            }
            if update_info.requires_approval {
                info!(
                    "Not auto-updating plugin {} to v{}: it requests new domains ({}) and needs approval",
                    update_info.plugin_id,
                    update_info.available_version,
                    update_info
                        .added_domains
                        .as_deref()
                        .unwrap_or_default()
                        .join(", ")
                );
                continue;
            }

            match self.update_plugin(&update_info.plugin_id, false).await {
                Ok(meta) => updated.push(meta),
                Err(e) => {
                    warn!("Failed to update plugin {}: {e}", update_info.plugin_id);
                }
            }
        }

        Ok(updated)
    }

    /// After installing/updating, make sure the plugin's *own* manifest does
    /// not claim more than the user approved (the registry entry is what was
    /// shown). A mismatch disables the plugin.
    async fn enforce_approved_domains(
        &self,
        meta: &PluginMeta,
        approved: Option<&[String]>,
    ) -> Result<(), crate::Error> {
        if let Some(extra) =
            permissions::widened_domains(approved, meta.permissions.domains.as_deref())
        {
            self.loader.set_enabled(&meta.id, false).await?;
            return Err(crate::Error::PermissionApprovalRequired(format!(
                "plugin {} declares domains beyond what was approved ({}); it has been disabled",
                meta.id,
                extra.join(", ")
            )));
        }
        Ok(())
    }

    /// List all available plugins from the registry (marketplace). Entries
    /// the running host cannot load are filtered out rather than offered.
    pub async fn list_available(&self) -> Result<Vec<RegistryPlugin>, crate::Error> {
        let index = registry::fetch_index(&self.client, &self.config).await?;
        Ok(index
            .plugins
            .into_iter()
            .filter(RegistryPlugin::is_compatible)
            .collect())
    }
}

/// Human-readable domain list for an approval prompt.
pub fn describe_domains(domains: Option<&[String]>) -> String {
    match domains {
        None => "ANY public host (the plugin declares no permissions.domains)".to_string(),
        Some([]) => "no hosts".to_string(),
        Some(d) => d.join(", "),
    }
}
