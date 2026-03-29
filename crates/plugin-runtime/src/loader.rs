//! Plugin discovery, loading, validation, and URL matching.
//!
//! Loads JavaScript (.js) and TypeScript (.ts) plugins via QuickJS-NG.
//! TypeScript files are transpiled to JavaScript via SWC before loading.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use regex::Regex;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::engine::{EngineConfig, PluginContext, PluginEngine};
use crate::host_api::{self, HostApi};
use crate::sandbox::SandboxLimits;
use crate::types::{DownloadPackage, PluginMeta, PostProcessContext, PostProcessResult};

/// A loaded, ready-to-execute plugin.
struct LoadedPlugin {
    meta: PluginMeta,
    context: PluginContext,
    url_regex: Regex,
}

/// Manages all loaded plugins.
pub struct PluginLoader {
    plugin_dir: PathBuf,
    plugins: Arc<Mutex<Vec<LoadedPlugin>>>,
    host_api: HostApi,
    sandbox_limits: SandboxLimits,
    engine: Arc<PluginEngine>,
}

impl PluginLoader {
    pub fn new(plugin_dir: PathBuf, sandbox_limits: SandboxLimits) -> Self {
        Self::new_with_host_api(plugin_dir, sandbox_limits, HostApi::new(20))
    }

    /// Create a PluginLoader with a pre-configured HostApi (for wiring callbacks).
    pub fn new_with_host_api(
        plugin_dir: PathBuf,
        sandbox_limits: SandboxLimits,
        host_api: HostApi,
    ) -> Self {
        let engine = PluginEngine::new(EngineConfig {
            max_memory: sandbox_limits.max_memory_bytes as usize,
            ..Default::default()
        })
        .expect("Failed to create QuickJS engine");

        Self {
            plugin_dir,
            plugins: Arc::new(Mutex::new(Vec::new())),
            host_api,
            sandbox_limits,
            engine: Arc::new(engine),
        }
    }

    /// Scan plugin directory and load all plugins.
    /// Supports category folders: plugins/<category>/<plugin-id>/plugin.ts
    /// Also supports flat: plugins/<plugin-id>/plugin.ts
    pub async fn discover(&self) -> Result<Vec<PluginMeta>, crate::Error> {
        let mut metas = Vec::new();
        let skip = ["types", "template"];

        let entries = match std::fs::read_dir(&self.plugin_dir) {
            Ok(e) => e,
            Err(_) => return Ok(metas),
        };

        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }

            let dir_name = dir.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if skip.contains(&dir_name) {
                continue;
            }

            // Check if this dir has a plugin.ts/js directly (flat structure)
            if let Some(path) = find_plugin_entry(&dir) {
                self.try_load_plugin(&path, &mut metas).await;
                continue;
            }

            // Otherwise treat as category dir — scan subdirs
            if let Ok(sub_entries) = std::fs::read_dir(&dir) {
                for sub_entry in sub_entries.flatten() {
                    let sub_dir = sub_entry.path();
                    if sub_dir.is_dir() {
                        if let Some(path) = find_plugin_entry(&sub_dir) {
                            self.try_load_plugin(&path, &mut metas).await;
                        }
                    }
                }
            }
        }

        info!("Discovered {} plugins", metas.len());
        Ok(metas)
    }

    /// Load a single .js/.ts plugin file, extract metadata, and register it.
    async fn load_plugin(&self, path: &Path) -> Result<PluginMeta, crate::Error> {
        let source_code = std::fs::read_to_string(path)
            .map_err(|e| crate::Error::Other(format!("Failed to read {}: {e}", path.display())))?;

        // TypeScript → JS: transpile via SWC
        let js_source = if crate::transpiler::is_typescript(path) {
            let filename = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("plugin.ts");
            crate::transpiler::transpile(&source_code, filename)?
        } else {
            source_code
        };

        // Create a QuickJS context for this plugin
        let context = self.engine.create_context()?;

        // Preliminary plugin ID from filename
        let temp_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        // Register host API
        context.with(|ctx| {
            host_api::register_host_api(&ctx, Arc::new(self.host_api.clone()), temp_id)
        })?;

        // Inject `module.exports` for CommonJS-style default export.
        // Plugin writes: module.exports = { pluginId() {}, resolve(url) {}, ... }
        // After eval, __plugin_exports points to module.exports.
        let wrapped = format!(
            r#"var module = {{ exports: {{}} }};
{js_source}
var __plugin_exports = module.exports;
"#
        );

        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("plugin.js");
        context.eval_source(&wrapped, filename)?;

        // Extract required metadata — supports both properties and functions
        let id = context.get_export_string("id")?;
        let name = context.get_export_string("name")?;
        let version = context.get_export_string("version")?;
        let url_pattern = context.get_export_string("urlPattern")?;

        // Validate required function: resolve
        context
            .require_export_function("resolve")
            .map_err(|e| crate::Error::Execution(format!("Plugin {id}: {e}")))?;

        let url_regex = Regex::new(&url_pattern).map_err(|e| {
            crate::Error::Execution(format!("Invalid urlPattern in plugin {id}: {e}"))
        })?;

        // Optional metadata
        let description = context.get_export_string("description").ok();
        let author = context.get_export_string("author").ok();

        let meta = PluginMeta {
            id: id.clone(),
            name,
            version,
            url_pattern,
            file_path: path.display().to_string(),
            enabled: true,
            description,
            author,
        };

        let mut plugins = self.plugins.lock().await;
        // Replace if already loaded (hot-reload)
        plugins.retain(|p| p.meta.id != id);
        plugins.push(LoadedPlugin {
            meta: meta.clone(),
            context,
            url_regex,
        });

        Ok(meta)
    }

    /// Find a plugin that matches the given URL.
    pub async fn match_url(&self, url: &str) -> Option<PluginMeta> {
        let plugins = self.plugins.lock().await;
        // Find most specific match (non-generic first)
        for plugin in plugins.iter() {
            if plugin.meta.enabled
                && plugin.meta.id != "generic-http"
                && plugin.url_regex.is_match(url)
            {
                return Some(plugin.meta.clone());
            }
        }
        // Fall back to generic-http
        for plugin in plugins.iter() {
            if plugin.meta.enabled
                && plugin.meta.id == "generic-http"
                && plugin.url_regex.is_match(url)
            {
                return Some(plugin.meta.clone());
            }
        }
        None
    }

    /// Execute a plugin's resolve() function for a URL.
    /// Returns a DownloadPackage with a name and one or more downloads.
    pub async fn resolve(
        &self,
        plugin_id: &str,
        url: &str,
    ) -> Result<DownloadPackage, crate::Error> {
        let plugins = self.plugins.lock().await;
        let plugin = plugins
            .iter()
            .find(|p| p.meta.id == plugin_id)
            .ok_or_else(|| crate::Error::NotFound(plugin_id.to_string()))?;

        self.host_api.reset_request_count().await;

        let timeout = std::time::Duration::from_secs(self.sandbox_limits.max_execution_secs);

        let json_result = plugin.context.call_resolve(url, timeout)?;

        let pkg: DownloadPackage = serde_json::from_str(&json_result).map_err(|e| {
            crate::Error::Execution(format!(
                "Plugin {plugin_id} returned invalid JSON: {e}\nGot: {json_result}"
            ))
        })?;

        debug!(
            "Plugin {plugin_id} resolved package '{}' with {} downloads",
            pkg.name,
            pkg.downloads.len()
        );
        Ok(pkg)
    }

    /// Execute a plugin's postProcess() function if it exists.
    pub async fn post_process(
        &self,
        plugin_id: &str,
        context: &PostProcessContext,
    ) -> Result<PostProcessResult, crate::Error> {
        let plugins = self.plugins.lock().await;
        let plugin = plugins
            .iter()
            .find(|p| p.meta.id == plugin_id)
            .ok_or_else(|| crate::Error::NotFound(plugin_id.to_string()))?;

        if !plugin.context.has_post_process() {
            return Ok(PostProcessResult {
                success: true,
                files_created: None,
                files_to_delete: None,
                message: Some("No postProcess hook".into()),
            });
        }

        let context_json = serde_json::to_string(context).map_err(|e| {
            crate::Error::Execution(format!("Failed to serialize PostProcessContext: {e}"))
        })?;

        let timeout = std::time::Duration::from_secs(self.sandbox_limits.max_execution_secs);
        let json_result = plugin.context.call_post_process(&context_json, timeout)?;

        let result: PostProcessResult = serde_json::from_str(&json_result).map_err(|e| {
            crate::Error::Execution(format!(
                "Plugin {plugin_id} postProcess returned invalid JSON: {e}\nGot: {json_result}"
            ))
        })?;

        debug!(
            "Plugin {plugin_id} postProcess: success={}, message={:?}",
            result.success, result.message
        );
        Ok(result)
    }

    /// List all loaded plugins.
    pub async fn list_plugins(&self) -> Vec<PluginMeta> {
        let plugins = self.plugins.lock().await;
        plugins.iter().map(|p| p.meta.clone()).collect()
    }

    /// Get metadata for a single plugin by ID.
    pub async fn get_plugin_meta(&self, plugin_id: &str) -> Option<PluginMeta> {
        let plugins = self.plugins.lock().await;
        plugins
            .iter()
            .find(|p| p.meta.id == plugin_id)
            .map(|p| p.meta.clone())
    }

    /// Get the plugin directory path.
    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
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

    /// Run a spec file against a loaded plugin.
    /// Looks for `<plugin>.spec.ts` or `<plugin>.spec.js` next to the plugin file.
    pub async fn run_spec(
        &self,
        plugin_id: &str,
    ) -> Result<crate::engine::TestResults, crate::Error> {
        let plugins = self.plugins.lock().await;
        let plugin = plugins
            .iter()
            .find(|p| p.meta.id == plugin_id)
            .ok_or_else(|| crate::Error::NotFound(plugin_id.to_string()))?;

        let plugin_path = PathBuf::from(&plugin.meta.file_path);
        let dir = plugin_path.parent().unwrap_or(Path::new("."));

        // Look for plugin.spec.ts or plugin.spec.js in the same directory
        let spec_path = ["plugin.spec.ts", "plugin.spec.js"]
            .iter()
            .map(|f| dir.join(f))
            .find(|p| p.exists())
            .ok_or_else(|| {
                crate::Error::NotFound(format!(
                    "No spec file found for plugin {plugin_id} (expected plugin.spec.ts or plugin.spec.js in {})",
                    dir.display()
                ))
            })?;

        let spec_source = std::fs::read_to_string(&spec_path).map_err(|e| {
            crate::Error::Other(format!("Failed to read {}: {e}", spec_path.display()))
        })?;

        let spec_source = if crate::transpiler::is_typescript(&spec_path) {
            let filename = spec_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("spec.ts");
            crate::transpiler::transpile(&spec_source, filename)?
        } else {
            spec_source
        };

        let filename = spec_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("spec.js");
        Ok(plugin.context.run_tests(&spec_source, filename))
    }

    async fn try_load_plugin(&self, path: &Path, metas: &mut Vec<PluginMeta>) {
        match self.load_plugin(path).await {
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

/// Find plugin.ts or plugin.js in a directory.
fn find_plugin_entry(dir: &Path) -> Option<PathBuf> {
    ["plugin.ts", "plugin.js"]
        .iter()
        .map(|f| dir.join(f))
        .find(|p| p.exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_load_and_match_js_plugin() {
        let dir = std::env::temp_dir().join("amigo-test-plugins-js2");
        let plugin_dir = dir.join("test-hoster");
        std::fs::create_dir_all(&plugin_dir).unwrap();

        std::fs::write(
            plugin_dir.join("plugin.js"),
            r#"
module.exports = {
    id: "test-hoster",
    name: "Test Hoster",
    version: "1.0.0",
    urlPattern: "https?://test-hoster\\.com/.+",
    resolve(url) { return { name: "Test", downloads: [{ url: url, filename: null, filesize: null, chunks_supported: true, max_chunks: 8, headers: null, cookies: null, wait_seconds: null, mirrors: [] }] }; },
};
"#,
        )
        .unwrap();

        let loader = PluginLoader::new(dir.clone(), SandboxLimits::default());
        let plugins = loader.discover().await.unwrap();

        assert!(
            !plugins.is_empty(),
            "Should discover at least one plugin. Dir: {:?}",
            dir
        );
        assert!(plugins.iter().any(|p| p.id == "test-hoster"));

        let matched = loader.match_url("https://test-hoster.com/file.zip").await;
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().id, "test-hoster");

        let no_match = loader.match_url("https://other-site.com/file.zip").await;
        assert!(no_match.is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn test_load_typescript_plugin() {
        let dir = std::env::temp_dir().join("amigo-test-plugins-ts2");
        let plugin_dir = dir.join("ts-hoster");
        std::fs::create_dir_all(&plugin_dir).unwrap();

        std::fs::write(
            plugin_dir.join("plugin.ts"),
            r#"
module.exports = {
    id: "ts-hoster",
    name: "TS Hoster",
    version: "2.0.0",
    urlPattern: "https?://ts-hoster\\.com/.+",
    resolve(url: string): DownloadPackage {
        return { name: "Test", downloads: [{ url: url, filename: null, filesize: null, chunks_supported: true, max_chunks: null, headers: null, cookies: null, wait_seconds: null, mirrors: [] }] };
    },
};
"#,
        )
        .unwrap();

        let loader = PluginLoader::new(dir.clone(), SandboxLimits::default());
        let plugins = loader.discover().await.unwrap();

        assert!(plugins.iter().any(|p| p.id == "ts-hoster"));
        assert_eq!(
            plugins
                .iter()
                .find(|p| p.id == "ts-hoster")
                .unwrap()
                .version,
            "2.0.0"
        );

        let matched = loader.match_url("https://ts-hoster.com/file.zip").await;
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().id, "ts-hoster");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn test_load_from_category_subfolder() {
        let dir = std::env::temp_dir().join("amigo-test-plugins-cat");
        let plugin_dir = dir.join("hosters").join("cat-hoster");
        std::fs::create_dir_all(&plugin_dir).unwrap();

        std::fs::write(
            plugin_dir.join("plugin.js"),
            r#"
module.exports = {
    id: "cat-hoster",
    name: "Category Hoster",
    version: "1.0.0",
    urlPattern: "https?://cat-hoster\\.com/.+",
    resolve(url) { return { name: "Test", downloads: [{ url: url, filename: null, filesize: null, chunks_supported: true, max_chunks: null, headers: null, cookies: null, wait_seconds: null, mirrors: [] }] }; },
};
"#,
        )
        .unwrap();

        let loader = PluginLoader::new(dir.clone(), SandboxLimits::default());
        let plugins = loader.discover().await.unwrap();

        assert!(plugins.iter().any(|p| p.id == "cat-hoster"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
