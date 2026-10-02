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

        let artifact = registry::fetch_plugin_artifact(&self.client, registry_plugin).await?;
        self.validate_candidate(
            registry_plugin,
            &artifact,
            registry_plugin.declared_domains(),
        )
        .await?;

        let plugin_dir = self.loader.plugin_dir().join(&registry_plugin.id);
        let path = registry::install_artifact(&plugin_dir, &artifact)?;
        self.loader.load_plugin(&path).await
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
        let approved_domains = if approved {
            registry_plugin.declared_domains()
        } else {
            installed_domains.as_deref()
        };

        let artifact = registry::fetch_plugin_artifact(&self.client, registry_plugin).await?;
        self.validate_candidate(registry_plugin, &artifact, approved_domains)
            .await?;

        // Replace the installed file in place (it may live in a category
        // folder such as plugins/hosters/<id>/), then hot-reload it.
        let installed_path = std::path::PathBuf::from(&installed.file_path);
        let plugin_dir = installed_path
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| self.loader.plugin_dir().join(plugin_id));
        let path = registry::install_artifact(&plugin_dir, &artifact)?;
        let meta = self.loader.load_plugin(&path).await?;
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

    /// Evaluate a downloaded artifact in a throw-away context — before
    /// anything is written into the plugin directory — and check that its own
    /// manifest matches the signed registry entry it was fetched for (id and
    /// version) and claims no domains beyond `approved_domains`. A candidate
    /// that fails never reaches disk, so nothing rejected can come back on
    /// the next start.
    async fn validate_candidate(
        &self,
        entry: &RegistryPlugin,
        artifact: &registry::PluginArtifact,
        approved_domains: Option<&[String]>,
    ) -> Result<(), crate::Error> {
        let staging = tempfile::tempdir()
            .map_err(|e| crate::Error::Other(format!("Failed to create staging dir: {e}")))?;
        let path = registry::install_artifact(staging.path(), artifact)?;
        let meta = self.loader.inspect_plugin(&path).await?;
        check_candidate(entry, &meta, approved_domains)
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

/// Check a candidate's own manifest against its registry entry and the
/// approved domains.
fn check_candidate(
    entry: &RegistryPlugin,
    meta: &PluginMeta,
    approved_domains: Option<&[String]>,
) -> Result<(), crate::Error> {
    if meta.id != entry.id || meta.version != entry.version {
        return Err(crate::Error::SandboxViolation(format!(
            "artifact for {} v{} declares itself as {} v{}; refusing to install",
            entry.id, entry.version, meta.id, meta.version
        )));
    }
    if let Some(extra) =
        permissions::widened_domains(approved_domains, meta.permissions.domains.as_deref())
    {
        return Err(crate::Error::PermissionApprovalRequired(format!(
            "plugin {} declares domains beyond what was approved ({}); not installed",
            meta.id,
            extra.join(", ")
        )));
    }
    Ok(())
}

/// Human-readable domain list for an approval prompt.
pub fn describe_domains(domains: Option<&[String]>) -> String {
    match domains {
        None => "ANY public host (the plugin declares no permissions.domains)".to_string(),
        Some([]) => "no hosts".to_string(),
        Some(d) => d.join(", "),
    }
}
