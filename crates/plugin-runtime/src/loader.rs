//! Plugin discovery, loading, validation, and URL matching.
//!
//! Loads JavaScript (.js) and TypeScript (.ts) plugins. The plugins' JS runs
//! in the `amigo-plugin-host` child process (see [`crate::plugin_host`]); this
//! side keeps the metadata, URL matching, enable flags, the per-plugin exec
//! locks and the scoped [`HostApi`]s, and holds no QuickJS runtime.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use regex::Regex;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

use crate::bytecode_cache::BytecodeCache;
use crate::host_api::{HostApi, base64_decode_bytes, base64_encode_bytes};
use crate::ipc::{Handle, Payload, Request};
use crate::plugin_host::{check_api_version, parse_allowlist};
use crate::process_host::{Child, ProcessHost};
use crate::sandbox::SandboxLimits;
use crate::types::{
    DownloadPackage, PluginMeta, PluginType, PostProcessContext, PostProcessResult,
};

/// A plugin's source, kept so the plugin can be re-loaded into a new plugin
/// host after the old one died.
struct PluginSource {
    /// Path shown in errors until the plugin's id is known.
    label: String,
    source: String,
    filename: String,
    is_typescript: bool,
}

/// Where a plugin is loaded: which plugin host generation, under which
/// handle. Generation 0 never matches a live host.
struct Slot {
    generation: u64,
    handle: Handle,
}

/// A loaded, ready-to-execute plugin.
struct LoadedPlugin {
    meta: PluginMeta,
    source: Arc<PluginSource>,
    /// The host API scoped to this plugin (own request counter, own domain
    /// allowlist).
    host: HostApi,
    /// Serializes invocations of this one plugin and records where it is
    /// loaded. Held across execution — unlike the plugin-list lock, which is
    /// only held to look a plugin up.
    slot: Arc<Mutex<Slot>>,
    url_regex: Regex,
}

/// What the loader shares with execution tasks.
struct Runtime {
    process: ProcessHost,
    sandbox_limits: SandboxLimits,
    bytecode_cache: Option<BytecodeCache>,
}

/// A plugin freshly loaded into a plugin host.
struct Prepared {
    child: Arc<Child>,
    handle: Handle,
    meta: PluginMeta,
    allowlist: Option<crate::permissions::DomainAllowlist>,
}

impl Runtime {
    /// Load `source` into the live plugin host (spawning one if needed),
    /// using and feeding the bytecode cache. Re-validates what the child
    /// reports instead of trusting it.
    async fn load(&self, source: &PluginSource) -> Result<Prepared, crate::Error> {
        let child = self.process.child().await?;
        let cache_key = self
            .bytecode_cache
            .as_ref()
            .map(|c| c.key(&source.filename, &source.source));
        let cached_bytecode = match (&self.bytecode_cache, &cache_key) {
            (Some(cache), Some(key)) => cache.get(key).map(|b| base64_encode_bytes(&b)),
            _ => None,
        };

        let request = Request::Load {
            source: source.source.clone(),
            filename: source.filename.clone(),
            is_typescript: source.is_typescript,
            cached_bytecode,
            want_bytecode: self.bytecode_cache.is_some(),
            limits: self.sandbox_limits.clone(),
        };
        // Module evaluation and each manifest read run under their own load
        // deadline inside the child; allow for all of them.
        let budget = Duration::from_secs(self.sandbox_limits.max_load_secs.max(1) * 16);
        let payload = child
            .request(
                request,
                &source.label,
                budget,
                self.process.watchdog_grace(),
            )
            .await?;
        let Payload::Loaded {
            handle,
            mut meta,
            new_bytecode,
            cached_bytecode_rejected,
            warnings,
        } = payload
        else {
            return Err(crate::Error::Execution(
                "plugin host answered a load with the wrong payload".into(),
            ));
        };

        if let (Some(cache), Some(key)) = (&self.bytecode_cache, &cache_key) {
            if cached_bytecode_rejected {
                warn!(
                    "Cached bytecode for {} failed; dropping the entry",
                    source.filename
                );
                cache.remove(key);
            }
            if let Some(b64) = new_bytecode
                && let Ok(bytecode) = base64_decode_bytes(&b64)
            {
                cache.put(key, &bytecode);
            }
        } else if !cached_bytecode_rejected {
            debug!("Loaded {} without a bytecode cache", source.filename);
        }
        for w in warnings {
            warn!("{w}");
        }

        let validated = (|| {
            check_api_version(meta.api_version)?;
            let allowlist = parse_allowlist(&meta.id, &meta.permissions)?;
            meta.failing = false;
            meta.enabled = true;
            Ok::<_, crate::Error>(allowlist)
        })();
        match validated {
            Ok(allowlist) => Ok(Prepared {
                child,
                handle,
                meta: *meta,
                allowlist,
            }),
            Err(e) => {
                child.drop_handle(handle);
                Err(e)
            }
        }
    }
}

/// Manages all loaded plugins.
///
/// The plugin list sits behind an `RwLock` that is only ever held long enough
/// to find a plugin and clone the handles execution needs. Plugin JS runs in
/// the plugin host process, never while the list lock is held — so
/// `list_plugins` / `match_url` stay responsive while a plugin is
/// mid-`resolve()`, and different plugins run concurrently, each in its own
/// QuickJS runtime under its own deadline.
pub struct PluginLoader {
    plugin_dir: PathBuf,
    plugins: Arc<RwLock<Vec<LoadedPlugin>>>,
    host_api: HostApi,
    runtime: Arc<Runtime>,
}

impl PluginLoader {
    pub fn new(plugin_dir: PathBuf, sandbox_limits: SandboxLimits) -> Result<Self, crate::Error> {
        let host_api = HostApi::from_sandbox(&sandbox_limits);
        Self::new_with_host_api(plugin_dir, sandbox_limits, host_api)
    }

    /// Create a PluginLoader with a pre-configured HostApi (for wiring callbacks).
    pub fn new_with_host_api(
        plugin_dir: PathBuf,
        sandbox_limits: SandboxLimits,
        host_api: HostApi,
    ) -> Result<Self, crate::Error> {
        Ok(Self {
            plugin_dir,
            plugins: Arc::new(RwLock::new(Vec::new())),
            host_api,
            runtime: Arc::new(Runtime {
                process: ProcessHost::new(None),
                sandbox_limits,
                bytecode_cache: None,
            }),
        })
    }

    fn runtime_mut(&mut self) -> &mut Runtime {
        Arc::get_mut(&mut self.runtime).expect("builder methods run before the loader is shared")
    }

    /// Persist compiled plugin bytecode in `cache` so later starts skip the
    /// transpile + compile step. See [`crate::bytecode_cache`] for the
    /// constraints on where the cache may live.
    pub fn with_bytecode_cache(mut self, cache: BytecodeCache) -> Self {
        self.runtime_mut().bytecode_cache = Some(cache);
        self
    }

    /// Run plugins with the `amigo-plugin-host` binary at `path` instead of
    /// looking it up (`$AMIGO_PLUGIN_HOST_BIN`, then next to the current
    /// executable).
    pub fn with_plugin_host_binary(mut self, path: PathBuf) -> Self {
        self.runtime_mut().process = ProcessHost::new(Some(path));
        self
    }

    /// PID of the running plugin host process, if any. For tests and
    /// diagnostics.
    #[doc(hidden)]
    pub async fn plugin_host_pid(&self) -> Option<u32> {
        self.runtime.process.pid().await
    }

    /// Override the grace the liveness watchdog adds to a call's budget
    /// before it kills an unresponsive plugin host. For tests.
    #[doc(hidden)]
    pub fn set_watchdog_grace(&self, grace: Duration) {
        self.runtime.process.set_watchdog_grace(grace);
    }

    /// Scan plugin directory and load all plugins.
    /// Supports category folders: plugins/<category>/<plugin-id>/plugin.ts
    /// Also supports flat: plugins/<plugin-id>/plugin.ts
    pub async fn discover(&self) -> Result<Vec<PluginMeta>, crate::Error> {
        let mut metas = Vec::new();
        let skip = ["types", "template"];

        let Ok(entries) = std::fs::read_dir(&self.plugin_dir) else {
            return Ok(metas);
        };

        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }

            let dir_name = dir.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if skip.contains(&dir_name) || dir_name.starts_with('.') {
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
                    if sub_dir.is_dir()
                        && let Some(path) = find_plugin_entry(&sub_dir)
                    {
                        self.try_load_plugin(&path, &mut metas).await;
                    }
                }
            }
        }

        info!("Discovered {} plugins", metas.len());
        Ok(metas)
    }

    /// Load a single .js/.ts plugin file, extract metadata, and register it.
    ///
    /// Module evaluation runs in the plugin host under the load deadline, so
    /// a plugin whose module body never returns fails to load instead of
    /// hanging `discover()`.
    pub async fn load_plugin(&self, path: &Path) -> Result<PluginMeta, crate::Error> {
        let prepared = self.prepare(path).await?;
        Ok(self.commit(prepared).await)
    }

    /// Evaluate a plugin file in a fresh, throw-away context and return its
    /// manifest **without registering it**. Used to validate a downloaded
    /// candidate (identity, version, permissions) before it is written into
    /// the plugin directory.
    pub async fn inspect_plugin(&self, path: &Path) -> Result<PluginMeta, crate::Error> {
        let (source, file_path) = read_source(path)?;
        let prepared = self.runtime.load(&source).await?;
        prepared.child.drop_handle(prepared.handle);
        let mut meta = prepared.meta;
        meta.file_path = file_path;
        Ok(meta)
    }

    /// Evaluate a plugin file into a ready-to-register [`LoadedPlugin`].
    async fn prepare(&self, path: &Path) -> Result<LoadedPlugin, crate::Error> {
        let (source, file_path) = read_source(path)?;
        let Prepared {
            child,
            handle,
            mut meta,
            allowlist,
        } = self.runtime.load(&source).await?;
        meta.file_path = file_path;
        let id = meta.id.clone();

        let url_regex = match Regex::new(&meta.url_pattern) {
            Ok(re) => re,
            Err(e) => {
                child.drop_handle(handle);
                return Err(crate::Error::Execution(format!(
                    "Invalid urlPattern in plugin {id}: {e}"
                )));
            }
        };
        if allowlist.is_none() {
            warn!("Plugin {id} declares no permissions.domains — it may reach any public host");
        }

        // Bind the host API to the plugin's *declared* id and declared domain
        // allowlist. Binding to the declared id (rather than the file stem,
        // which is always "plugin") is what isolates each plugin's cookie jar
        // and storage namespace from every other plugin; binding the
        // allowlist here, daemon-side, is what keeps JS — and the plugin
        // host — from choosing which list applies.
        let host = self.host_api.scoped(allowlist);
        child.register(handle, &id, host.clone());

        Ok(LoadedPlugin {
            meta,
            source: Arc::new(source),
            host,
            slot: Arc::new(Mutex::new(Slot {
                generation: child.generation,
                handle,
            })),
            url_regex,
        })
    }

    /// Register a prepared plugin, replacing any loaded plugin with the same
    /// id (hot-reload).
    async fn commit(&self, plugin: LoadedPlugin) -> PluginMeta {
        let meta = plugin.meta.clone();
        let replaced: Vec<Arc<Mutex<Slot>>> = {
            let mut plugins = self.plugins.write().await;
            let (old, keep): (Vec<_>, Vec<_>) =
                plugins.drain(..).partition(|p| p.meta.id == meta.id);
            *plugins = keep;
            plugins.push(plugin);
            old.into_iter().map(|p| p.slot).collect()
        };
        // Free the replaced contexts once their in-flight calls finish.
        for slot in replaced {
            let slot = slot.lock().await;
            if let Some(child) = self.runtime.process.live().await
                && child.generation == slot.generation
            {
                child.drop_handle(slot.handle);
            }
        }
        meta
    }

    /// Find a plugin that matches the given URL.
    ///
    /// Priority order:
    /// 1. Multi-hoster plugins (Real-Debrid, Premiumize, etc.) — when configured
    /// 2. Site-specific hoster/extractor plugins
    /// 3. Generic fallback plugins (generic-http, generic-media)
    pub async fn match_url(&self, url: &str) -> Option<PluginMeta> {
        let plugins = self.plugins.read().await;

        // Pass 1: multi-hoster plugins (highest priority when available)
        for plugin in plugins.iter() {
            if plugin.meta.enabled
                && plugin.meta.plugin_type == PluginType::MultiHoster
                && plugin.url_regex.is_match(url)
            {
                return Some(plugin.meta.clone());
            }
        }

        // Pass 2: site-specific plugins
        for plugin in plugins.iter() {
            if plugin.meta.enabled
                && plugin.meta.plugin_type == PluginType::Hoster
                && plugin.url_regex.is_match(url)
            {
                return Some(plugin.meta.clone());
            }
        }

        // Pass 3: generic fallback plugins
        for plugin in plugins.iter() {
            if plugin.meta.enabled
                && plugin.meta.plugin_type == PluginType::Generic
                && plugin.url_regex.is_match(url)
            {
                return Some(plugin.meta.clone());
            }
        }

        None
    }

    /// Run one request against plugin `plugin_id` in the plugin host.
    ///
    /// The plugin-list lock is released before execution starts; only this
    /// plugin's own slot lock is held for the duration. The call runs in its
    /// own task that owns the lock, so it stays held until the plugin host
    /// answers even if the caller's future is dropped mid-call. A plugin
    /// whose host died is re-loaded into the new one first.
    async fn execute(
        &self,
        plugin_id: &str,
        budget: Duration,
        request: impl FnOnce(Handle) -> Request + Send + 'static,
    ) -> Result<Payload, crate::Error> {
        let (slot, host, source) = {
            let plugins = self.plugins.read().await;
            let plugin = plugins
                .iter()
                .find(|p| p.meta.id == plugin_id)
                .ok_or_else(|| crate::Error::NotFound(plugin_id.to_string()))?;
            (
                Arc::clone(&plugin.slot),
                plugin.host.clone(),
                Arc::clone(&plugin.source),
            )
        };

        let mut slot = slot.lock_owned().await;
        host.reset_request_count().await;

        let runtime = Arc::clone(&self.runtime);
        let id = plugin_id.to_string();
        let result = tokio::spawn(async move {
            let child = runtime.process.child().await?;
            if slot.generation != child.generation {
                debug!(
                    "Re-loading plugin {id} into plugin host generation {}",
                    child.generation
                );
                let prepared = runtime.load(&source).await?;
                if prepared.meta.id != id {
                    prepared.child.drop_handle(prepared.handle);
                    return Err(crate::Error::Execution(format!(
                        "plugin {id} re-loaded as {:?}",
                        prepared.meta.id
                    )));
                }
                prepared.child.register(prepared.handle, &id, host);
                slot.generation = prepared.child.generation;
                slot.handle = prepared.handle;
            }
            let grace = runtime.process.watchdog_grace();
            let child = runtime.process.child().await?;
            if child.generation != slot.generation {
                return Err(crate::Error::HostCrashed(format!(
                    "plugin host restarted while re-loading plugin {id}"
                )));
            }
            child
                .request(request(slot.handle), &id, budget, grace)
                .await
        })
        .await
        .map_err(|e| crate::Error::Execution(format!("plugin task failed: {e}")))?;

        self.record_health(plugin_id, &result).await;
        result
    }

    /// Mark a plugin `failing` when the plugin host died under it; clear the
    /// mark on its next successful call.
    async fn record_health(&self, plugin_id: &str, result: &Result<Payload, crate::Error>) {
        let crashed = matches!(result, Err(crate::Error::HostCrashed(_)));
        let changed = {
            let plugins = self.plugins.read().await;
            plugins
                .iter()
                .find(|p| p.meta.id == plugin_id)
                .is_some_and(|p| p.meta.failing != crashed)
        };
        if crashed && let Err(e) = result {
            self.host_api.notify(
                plugin_id,
                "Plugin crashed",
                &format!("{e}. The plugin host was restarted."),
            );
        }
        if !changed || (!crashed && result.is_err()) {
            return;
        }
        let mut plugins = self.plugins.write().await;
        if let Some(p) = plugins.iter_mut().find(|p| p.meta.id == plugin_id) {
            p.meta.failing = crashed;
        }
    }

    fn execution_timeout(&self) -> Duration {
        Duration::from_secs(self.runtime.sandbox_limits.max_execution_secs)
    }

    /// Execute a plugin's resolve() function for a URL.
    /// Returns a DownloadPackage with a name and one or more downloads.
    pub async fn resolve(
        &self,
        plugin_id: &str,
        url: &str,
    ) -> Result<DownloadPackage, crate::Error> {
        let timeout = self.execution_timeout();
        let url = url.to_string();
        let payload = self
            .execute(plugin_id, timeout, move |handle| Request::Resolve {
                handle,
                url,
                timeout_ms: timeout.as_millis() as u64,
            })
            .await?;
        let json_result = expect_json(payload)?.unwrap_or_default();

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
        let context_json = serde_json::to_string(context).map_err(|e| {
            crate::Error::Execution(format!("Failed to serialize PostProcessContext: {e}"))
        })?;
        let timeout = self.execution_timeout();

        let payload = self
            .execute(plugin_id, timeout, move |handle| Request::PostProcess {
                handle,
                context_json,
                timeout_ms: timeout.as_millis() as u64,
            })
            .await?;

        let Some(json_result) = expect_json(payload)? else {
            return Ok(PostProcessResult {
                success: true,
                files_created: None,
                files_to_delete: None,
                message: Some("No postProcess hook".into()),
            });
        };

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
        let plugins = self.plugins.read().await;
        plugins.iter().map(|p| p.meta.clone()).collect()
    }

    /// Get metadata for a single plugin by ID.
    pub async fn get_plugin_meta(&self, plugin_id: &str) -> Option<PluginMeta> {
        let plugins = self.plugins.read().await;
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
        let mut plugins = self.plugins.write().await;
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
            let plugins = self.plugins.read().await;
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
        let plugin_path = self
            .get_plugin_meta(plugin_id)
            .await
            .map(|m| PathBuf::from(m.file_path))
            .ok_or_else(|| crate::Error::NotFound(plugin_id.to_string()))?;
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
        let is_typescript = crate::transpiler::is_typescript(&spec_path);
        let filename = spec_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(if is_typescript { "spec.ts" } else { "spec.js" })
            .to_string();

        let timeout = self.execution_timeout();
        // The harness and spec body evaluate under the load deadline before
        // the tests run under `timeout`.
        let load = Duration::from_secs(self.runtime.sandbox_limits.max_load_secs.max(1));
        let budget = timeout.saturating_add(load * 2);
        let payload = self
            .execute(plugin_id, budget, move |handle| Request::RunSpec {
                handle,
                spec_source,
                filename,
                is_typescript,
                timeout_ms: timeout.as_millis() as u64,
            })
            .await?;
        match payload {
            Payload::Spec { results } => Ok(results),
            _ => Err(crate::Error::Execution(
                "plugin host answered a spec run with the wrong payload".into(),
            )),
        }
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

fn expect_json(payload: Payload) -> Result<Option<String>, crate::Error> {
    match payload {
        Payload::Json { json } => Ok(json),
        _ => Err(crate::Error::Execution(
            "plugin host answered with the wrong payload".into(),
        )),
    }
}

/// Read a plugin file for loading. Returns the source and its display path.
fn read_source(path: &Path) -> Result<(PluginSource, String), crate::Error> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| crate::Error::Other(format!("Failed to read {}: {e}", path.display())))?;
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("plugin.js")
        .to_string();
    Ok((
        PluginSource {
            label: path.display().to_string(),
            source,
            filename,
            is_typescript: crate::transpiler::is_typescript(path),
        },
        path.display().to_string(),
    ))
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

        let loader = PluginLoader::new(dir.clone(), SandboxLimits::default()).unwrap();
        let plugins = loader.discover().await.unwrap();

        assert!(
            !plugins.is_empty(),
            "Should discover at least one plugin. Dir: {dir:?}"
        );
        assert!(plugins.iter().any(|p| p.id == "test-hoster"));

        let matched = loader.match_url("https://test-hoster.com/file.zip").await;
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().id, "test-hoster");

        let no_match = loader.match_url("https://other-site.com/file.zip").await;
        assert!(no_match.is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_plugin_storage_is_isolated_by_declared_id() {
        // Two plugins whose entry files are both named `plugin.js` (as every
        // real plugin's is). Each writes a secret into per-plugin storage from
        // resolve(). Before the fix the host API was bound to the file stem
        // ("plugin") for both, so they shared one namespace and could read each
        // other's secrets; now it is bound to the declared plugin id.
        let dir = std::env::temp_dir().join("amigo-test-plugin-isolation");
        std::fs::remove_dir_all(&dir).ok();
        for (folder, id) in [("alpha", "alpha-hoster"), ("beta", "beta-hoster")] {
            let plugin_dir = dir.join(folder);
            std::fs::create_dir_all(&plugin_dir).unwrap();
            std::fs::write(
                plugin_dir.join("plugin.js"),
                format!(
                    r#"
module.exports = {{
    id: "{id}",
    name: "{id}",
    version: "1.0.0",
    urlPattern: "https?://{folder}\\.com/.+",
    resolve(url) {{
        amigo.storageSet("secret", "{id}-value");
        return {{ name: "x", downloads: [{{ url: url, filename: null, filesize: null, chunks_supported: true, max_chunks: null, headers: null, cookies: null, wait_seconds: null, mirrors: [] }}] }};
    }},
}};
"#
                ),
            )
            .unwrap();
        }

        let loader = PluginLoader::new(dir.clone(), SandboxLimits::default()).unwrap();
        loader.discover().await.unwrap();

        loader
            .resolve("alpha-hoster", "https://alpha.com/f")
            .await
            .unwrap();
        loader
            .resolve("beta-hoster", "https://beta.com/f")
            .await
            .unwrap();

        let api = loader.host_api();
        // Each plugin's write lands under its own declared id ...
        assert_eq!(
            api.storage_get("alpha-hoster", "secret").await.as_deref(),
            Some("alpha-hoster-value")
        );
        assert_eq!(
            api.storage_get("beta-hoster", "secret").await.as_deref(),
            Some("beta-hoster-value")
        );
        // ... and nothing leaks into the old shared "plugin" namespace.
        assert_eq!(api.storage_get("plugin", "secret").await, None);

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

        let loader = PluginLoader::new(dir.clone(), SandboxLimits::default()).unwrap();
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

        let loader = PluginLoader::new(dir.clone(), SandboxLimits::default()).unwrap();
        let plugins = loader.discover().await.unwrap();

        assert!(plugins.iter().any(|p| p.id == "cat-hoster"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
