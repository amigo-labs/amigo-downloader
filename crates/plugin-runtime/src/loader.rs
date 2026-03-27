//! Plugin discovery, loading, validation, and URL matching.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use regex::Regex;
use rune::runtime::RuntimeContext;
use rune::{Context, Diagnostics, Source, Sources, Unit, Vm};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::host_api::HostApi;
use crate::sandbox::SandboxLimits;
use crate::types::{DownloadInfo, PluginMeta};

/// A loaded, ready-to-execute plugin.
struct LoadedPlugin {
    meta: PluginMeta,
    unit: Arc<Unit>,
    url_regex: Regex,
}

/// Manages all loaded plugins.
pub struct PluginLoader {
    plugin_dir: PathBuf,
    plugins: Arc<Mutex<Vec<LoadedPlugin>>>,
    host_api: HostApi,
    sandbox_limits: SandboxLimits,
    context: Arc<Context>,
    runtime: Arc<RuntimeContext>,
}

impl PluginLoader {
    pub fn new(plugin_dir: PathBuf, sandbox_limits: SandboxLimits) -> Self {
        let context = Context::with_default_modules().expect("Failed to create Rune context");
        let runtime = context.runtime().expect("Failed to create runtime context");

        Self {
            plugin_dir,
            plugins: Arc::new(Mutex::new(Vec::new())),
            host_api: HostApi::new(sandbox_limits.max_http_requests),
            sandbox_limits,
            context: Arc::new(context),
            runtime: Arc::new(runtime),
        }
    }

    /// Scan plugin directory and load all .rn files.
    pub async fn discover(&self) -> Result<Vec<PluginMeta>, crate::Error> {
        let mut metas = Vec::new();

        // Scan hosters/ subdirectory
        let hosters_dir = self.plugin_dir.join("hosters");
        if hosters_dir.is_dir() {
            metas.extend(self.scan_dir(&hosters_dir).await?);
        }

        // Also scan root plugin dir
        metas.extend(self.scan_dir(&self.plugin_dir).await?);

        info!("Discovered {} plugins", metas.len());
        Ok(metas)
    }

    async fn scan_dir(&self, dir: &Path) -> Result<Vec<PluginMeta>, crate::Error> {
        let mut metas = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Ok(metas),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "rn") {
                match self.load_plugin(&path).await {
                    Ok(meta) => {
                        info!("Loaded plugin: {} ({})", meta.name, meta.id);
                        metas.push(meta);
                    }
                    Err(e) => {
                        warn!("Failed to load plugin {:?}: {e}", path);
                    }
                }
            }
        }

        Ok(metas)
    }

    /// Load a single .rn plugin file, extract metadata, and register it.
    async fn load_plugin(&self, path: &Path) -> Result<PluginMeta, crate::Error> {
        let source_code = std::fs::read_to_string(path)
            .map_err(|e| crate::Error::Other(format!("Failed to read {}: {e}", path.display())))?;

        let mut sources = Sources::new();
        sources
            .insert(Source::new(path.display().to_string(), &source_code).map_err(|e| crate::Error::Other(e.to_string()))?)
            .map_err(|e| crate::Error::Other(e.to_string()))?;

        let mut diagnostics = Diagnostics::new();
        let result = rune::prepare(&mut sources)
            .with_context(&self.context)
            .with_diagnostics(&mut diagnostics)
            .build();

        let unit = match result {
            Ok(unit) => unit,
            Err(e) => {
                return Err(crate::Error::Execution(format!(
                    "Compilation failed for {}: {e}",
                    path.display()
                )));
            }
        };

        let unit = Arc::new(unit);

        // Extract metadata by calling the metadata functions
        let mut vm = Vm::new(self.runtime.clone(), unit.clone());

        let id = call_string_fn(&mut vm, "plugin_id")?;
        let name = call_string_fn(&mut vm, "plugin_name")?;
        let version = call_string_fn(&mut vm, "plugin_version")?;
        let url_pattern = call_string_fn(&mut vm, "url_pattern")?;

        let url_regex = Regex::new(&url_pattern).map_err(|e| {
            crate::Error::Execution(format!(
                "Invalid url_pattern in plugin {id}: {e}"
            ))
        })?;

        let meta = PluginMeta {
            id: id.clone(),
            name,
            version,
            url_pattern,
            file_path: path.display().to_string(),
            enabled: true,
        };

        let mut plugins = self.plugins.lock().await;
        // Replace if already loaded (hot-reload)
        plugins.retain(|p| p.meta.id != id);
        plugins.push(LoadedPlugin {
            meta: meta.clone(),
            unit,
            url_regex,
        });

        Ok(meta)
    }

    /// Find a plugin that matches the given URL.
    pub async fn match_url(&self, url: &str) -> Option<PluginMeta> {
        let plugins = self.plugins.lock().await;
        // Find most specific match (non-generic first)
        for plugin in plugins.iter() {
            if plugin.meta.enabled && plugin.meta.id != "generic-http" && plugin.url_regex.is_match(url) {
                return Some(plugin.meta.clone());
            }
        }
        // Fall back to generic-http
        for plugin in plugins.iter() {
            if plugin.meta.enabled && plugin.meta.id == "generic-http" && plugin.url_regex.is_match(url) {
                return Some(plugin.meta.clone());
            }
        }
        None
    }

    /// Execute a plugin's resolve() function for a URL.
    pub async fn resolve(&self, plugin_id: &str, url: &str) -> Result<DownloadInfo, crate::Error> {
        let plugins = self.plugins.lock().await;
        let plugin = plugins
            .iter()
            .find(|p| p.meta.id == plugin_id)
            .ok_or_else(|| crate::Error::NotFound(plugin_id.to_string()))?;

        let mut vm = Vm::new(self.runtime.clone(), plugin.unit.clone());
        self.host_api.reset_request_count().await;

        let timeout = tokio::time::Duration::from_secs(self.sandbox_limits.max_execution_secs);

        let execution = vm.execute(["resolve"], (url.to_string(),));
        let mut execution = match execution {
            Ok(e) => e,
            Err(e) => {
                return Err(crate::Error::Execution(format!(
                    "Failed to call resolve(): {e}"
                )));
            }
        };

        // Run with timeout
        let vm_result = tokio::time::timeout(timeout, async {
            execution.async_complete().await
        })
        .await
        .map_err(|_| crate::Error::Timeout(self.sandbox_limits.max_execution_secs))?;

        let result = vm_result
            .into_result()
            .map_err(|e| crate::Error::Execution(format!("Plugin execution error: {e}")))?;

        // Convert Rune Value to DownloadInfo
        // For now, return a basic DownloadInfo — full conversion requires Rune value introspection
        debug!("Plugin {plugin_id} resolve returned: {:?}", result);

        // TODO: Proper Rune Value → DownloadInfo conversion
        // For now, plugins that just return the URL work via the generic path
        Ok(DownloadInfo {
            url: url.to_string(),
            filename: String::new(),
            filesize: None,
            chunks_supported: true,
            max_chunks: Some(8),
            headers: None,
            cookies: None,
            wait_seconds: None,
            mirrors: Vec::new(),
        })
    }

    /// List all loaded plugins.
    pub async fn list_plugins(&self) -> Vec<PluginMeta> {
        let plugins = self.plugins.lock().await;
        plugins.iter().map(|p| p.meta.clone()).collect()
    }

    /// Enable or disable a plugin.
    pub async fn set_enabled(&self, plugin_id: &str, enabled: bool) -> Result<(), crate::Error> {
        let mut plugins = self.plugins.lock().await;
        if let Some(plugin) = plugins.iter_mut().find(|p| p.meta.id == plugin_id) {
            plugin.meta.enabled = enabled;
            info!(
                "Plugin {} {}",
                plugin_id,
                if enabled { "enabled" } else { "disabled" }
            );
            Ok(())
        } else {
            Err(crate::Error::NotFound(plugin_id.to_string()))
        }
    }

    /// Reload a specific plugin from disk.
    pub async fn reload(&self, plugin_id: &str) -> Result<PluginMeta, crate::Error> {
        let path = {
            let plugins = self.plugins.lock().await;
            plugins
                .iter()
                .find(|p| p.meta.id == plugin_id)
                .map(|p| PathBuf::from(&p.meta.file_path))
                .ok_or_else(|| crate::Error::NotFound(plugin_id.to_string()))?
        };
        self.load_plugin(&path).await
    }

    /// Get the Host API (for use by the coordinator or server).
    pub fn host_api(&self) -> &HostApi {
        &self.host_api
    }
}

/// Call a simple string-returning function on a Rune VM.
fn call_string_fn(vm: &mut Vm, name: &str) -> Result<String, crate::Error> {
    let result = vm
        .call([name], ())
        .map_err(|e| crate::Error::Execution(format!("Failed to call {name}(): {e}")))?;

    let s: String = rune::from_value(result)
        .map_err(|e| crate::Error::Execution(format!("{name}() did not return a string: {e}")))?;

    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_load_and_match_plugin() {
        // Create a temp plugin that only uses metadata functions (no host API calls)
        let dir = std::env::temp_dir().join("amigo-test-plugins");
        let hosters = dir.join("hosters");
        std::fs::create_dir_all(&hosters).unwrap();

        std::fs::write(
            hosters.join("test_hoster.rn"),
            r#"
pub fn plugin_id() { "test-hoster" }
pub fn plugin_name() { "Test Hoster" }
pub fn plugin_version() { "1.0.0" }
pub fn url_pattern() { "https?://test-hoster\\.com/.+" }
pub async fn resolve(url) { Ok(#{url: url}) }
"#,
        )
        .unwrap();

        let loader = PluginLoader::new(dir.clone(), SandboxLimits::default());
        let plugins = loader.discover().await.unwrap();

        assert!(!plugins.is_empty(), "Should discover at least one plugin. Dir: {:?}", dir);
        assert!(plugins.iter().any(|p| p.id == "test-hoster"));

        let matched = loader.match_url("https://test-hoster.com/file.zip").await;
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().id, "test-hoster");

        // Non-matching URL should not match
        let no_match = loader.match_url("https://other-site.com/file.zip").await;
        assert!(no_match.is_none());

        // Cleanup
        std::fs::remove_dir_all(&dir).ok();
    }
}
