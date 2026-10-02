//! Plugin discovery, loading, validation, and URL matching.
//!
//! Loads JavaScript (.js) and TypeScript (.ts) plugins via QuickJS-NG.
//! TypeScript files are transpiled to JavaScript via SWC before loading.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use regex::Regex;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

use crate::bytecode_cache::BytecodeCache;
use crate::engine::{EngineConfig, PluginContext, PluginEngine};
use crate::host_api::{self, HostApi};
use crate::permissions::DomainAllowlist;
use crate::sandbox::SandboxLimits;
use crate::types::{
    DownloadPackage, PluginMeta, PluginPermissions, PluginType, PostProcessContext,
    PostProcessResult,
};

/// A loaded, ready-to-execute plugin.
struct LoadedPlugin {
    meta: PluginMeta,
    context: Arc<PluginContext>,
    /// The host API scoped to this plugin (own request counter, own domain
    /// allowlist).
    host: HostApi,
    /// Serializes invocations of this one plugin. Held across execution —
    /// unlike the plugin-list lock, which is only held to look a plugin up.
    exec: Arc<Mutex<()>>,
    url_regex: Regex,
}

/// Manages all loaded plugins.
///
/// The plugin list sits behind an `RwLock` that is only ever held long enough
/// to find a plugin and clone the handles execution needs. Plugin JS runs on
/// tokio's blocking pool (`spawn_blocking`), never on an async worker, and
/// never while the list lock is held — so `list_plugins` / `match_url` stay
/// responsive while a plugin is mid-`resolve()`, and different plugins run
/// concurrently, each in its own QuickJS runtime under its own deadline.
pub struct PluginLoader {
    plugin_dir: PathBuf,
    plugins: Arc<RwLock<Vec<LoadedPlugin>>>,
    host_api: HostApi,
    sandbox_limits: SandboxLimits,
    engine: Arc<PluginEngine>,
    bytecode_cache: Option<BytecodeCache>,
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
        let engine = PluginEngine::new(EngineConfig {
            max_memory: sandbox_limits.max_memory_bytes as usize,
            load_timeout: Duration::from_secs(sandbox_limits.max_load_secs.max(1)),
            ..Default::default()
        })?;

        Ok(Self {
            plugin_dir,
            plugins: Arc::new(RwLock::new(Vec::new())),
            host_api,
            sandbox_limits,
            engine: Arc::new(engine),
            bytecode_cache: None,
        })
    }

    /// Persist compiled plugin bytecode in `cache` so later starts skip the
    /// transpile + compile step. See [`crate::bytecode_cache`] for the
    /// constraints on where the cache may live.
    pub fn with_bytecode_cache(mut self, cache: BytecodeCache) -> Self {
        self.bytecode_cache = Some(cache);
        self
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
    /// Module evaluation runs on the blocking pool under the load deadline, so
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
        Ok(self.prepare(path).await?.meta)
    }

    /// Evaluate a plugin file into a ready-to-register [`LoadedPlugin`].
    async fn prepare(&self, path: &Path) -> Result<LoadedPlugin, crate::Error> {
        let source_code = std::fs::read_to_string(path)
            .map_err(|e| crate::Error::Other(format!("Failed to read {}: {e}", path.display())))?;

        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("plugin.js")
            .to_string();
        let file_path = path.display().to_string();
        let is_ts = crate::transpiler::is_typescript(path);
        let engine = Arc::clone(&self.engine);
        let cache = self.bytecode_cache.clone();
        let base_host = self.host_api.clone();

        let (meta, context, host, url_regex) = tokio::task::spawn_blocking(move || {
            let mut context = engine.create_context()?;
            match eval_cached(&context, cache.as_ref(), &source_code, &filename) {
                CachedEval::Loaded => {}
                CachedEval::Miss => {
                    eval_from_source(&context, cache.as_ref(), &source_code, &filename, is_ts)?
                }
                CachedEval::Failed(e @ crate::Error::Timeout(_)) => return Err(e),
                CachedEval::Failed(_) => {
                    // Start over in a clean context; a genuinely broken module
                    // body fails again here and is reported as such.
                    context = engine.create_context()?;
                    eval_from_source(&context, cache.as_ref(), &source_code, &filename, is_ts)?
                }
            }
            let (meta, allowlist) = read_manifest(&context, file_path)?;
            let id = meta.id.clone();

            // Register the host API bound to the plugin's *declared* id and
            // declared domain allowlist. This happens after evaluating the
            // module (so we know the real id and permissions) but before
            // `resolve()` or any callback runs — the module body itself only
            // defines exports and never touches `amigo`. Binding to the
            // declared id (rather than the file stem, which is always
            // "plugin") is what isolates each plugin's cookie jar and storage
            // namespace from every other plugin; binding the allowlist here is
            // what keeps JS from choosing which list applies.
            let host = base_host.scoped(allowlist);
            context.with(|ctx| host_api::register_host_api(&ctx, Arc::new(host.clone()), &id))?;

            // Validate required function: resolve
            context
                .require_export_function("resolve")
                .map_err(|e| crate::Error::Execution(format!("Plugin {id}: {e}")))?;

            let url_regex = Regex::new(&meta.url_pattern).map_err(|e| {
                crate::Error::Execution(format!("Invalid urlPattern in plugin {id}: {e}"))
            })?;
            Ok::<_, crate::Error>((meta, context, host, url_regex))
        })
        .await
        .map_err(|e| crate::Error::Execution(format!("plugin load task failed: {e}")))??;

        Ok(LoadedPlugin {
            meta,
            context: Arc::new(context),
            host,
            exec: Arc::new(Mutex::new(())),
            url_regex,
        })
    }

    /// Register a prepared plugin, replacing any loaded plugin with the same
    /// id (hot-reload).
    async fn commit(&self, plugin: LoadedPlugin) -> PluginMeta {
        let meta = plugin.meta.clone();
        let mut plugins = self.plugins.write().await;
        plugins.retain(|p| p.meta.id != meta.id);
        plugins.push(plugin);
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

    /// Run `f` against plugin `plugin_id`'s context on the blocking pool.
    ///
    /// The plugin-list lock is released before execution starts; only this
    /// plugin's own `exec` lock is held for the duration, and it is moved into
    /// the blocking task so it stays held even if the caller's future is
    /// dropped mid-call.
    async fn execute<T, F>(&self, plugin_id: &str, f: F) -> Result<T, crate::Error>
    where
        T: Send + 'static,
        F: FnOnce(&PluginContext) -> Result<T, crate::Error> + Send + 'static,
    {
        let (context, host, exec) = {
            let plugins = self.plugins.read().await;
            let plugin = plugins
                .iter()
                .find(|p| p.meta.id == plugin_id)
                .ok_or_else(|| crate::Error::NotFound(plugin_id.to_string()))?;
            (
                Arc::clone(&plugin.context),
                plugin.host.clone(),
                Arc::clone(&plugin.exec),
            )
        };

        let guard = exec.lock_owned().await;
        host.reset_request_count().await;

        tokio::task::spawn_blocking(move || {
            let _guard = guard;
            f(&context)
        })
        .await
        .map_err(|e| crate::Error::Execution(format!("plugin task failed: {e}")))?
    }

    /// Execute a plugin's resolve() function for a URL.
    /// Returns a DownloadPackage with a name and one or more downloads.
    pub async fn resolve(
        &self,
        plugin_id: &str,
        url: &str,
    ) -> Result<DownloadPackage, crate::Error> {
        let timeout = Duration::from_secs(self.sandbox_limits.max_execution_secs);
        let url = url.to_string();
        let json_result = self
            .execute(plugin_id, move |ctx| ctx.call_resolve(&url, timeout))
            .await?;

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
        let timeout = Duration::from_secs(self.sandbox_limits.max_execution_secs);

        let json_result = self
            .execute(plugin_id, move |ctx| {
                if !ctx.has_post_process() {
                    return Ok(None);
                }
                ctx.call_post_process(&context_json, timeout).map(Some)
            })
            .await?;

        let Some(json_result) = json_result else {
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
            .unwrap_or("spec.js")
            .to_string();
        let timeout = Duration::from_secs(self.sandbox_limits.max_execution_secs);
        self.execute(plugin_id, move |ctx| {
            Ok(ctx.run_tests(&spec_source, &filename, timeout))
        })
        .await
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

/// Outcome of trying to evaluate a plugin's module body from cached bytecode.
enum CachedEval {
    /// No usable entry — evaluate from source.
    Miss,
    /// Module body evaluated from bytecode.
    Loaded,
    /// The entry existed but failed; the context may be half-initialised.
    Failed(crate::Error),
}

/// Evaluate a plugin's module body from the bytecode cache, if it has an
/// entry for exactly this source under the current engine.
fn eval_cached(
    context: &PluginContext,
    cache: Option<&BytecodeCache>,
    source_code: &str,
    filename: &str,
) -> CachedEval {
    let Some(cache) = cache else {
        return CachedEval::Miss;
    };
    // The key covers the raw file contents, so a hit skips SWC as well as the
    // QuickJS compile.
    let key = cache.key(filename, source_code);
    let Some(bytecode) = cache.get(&key) else {
        return CachedEval::Miss;
    };
    // SAFETY: the entry was produced by `compile_to_bytecode` on this machine,
    // under a key that pins the engine version, and its checksum was verified
    // by `BytecodeCache::get`.
    match unsafe { context.eval_bytecode(&bytecode, filename) } {
        Ok(()) => {
            debug!("Loaded {filename} from bytecode cache");
            CachedEval::Loaded
        }
        Err(e) => {
            if !matches!(e, crate::Error::Timeout(_)) {
                warn!("Cached bytecode for {filename} failed ({e}); dropping the entry");
                cache.remove(&key);
            }
            CachedEval::Failed(e)
        }
    }
}

/// Evaluate a plugin's module body from source: transpile (TS), wrap, compile
/// and run — storing the bytecode in `cache` for the next load.
fn eval_from_source(
    context: &PluginContext,
    cache: Option<&BytecodeCache>,
    source_code: &str,
    filename: &str,
    is_ts: bool,
) -> Result<(), crate::Error> {
    // TypeScript → JS: transpile via SWC
    let js_source = if is_ts {
        crate::transpiler::transpile(source_code, filename)?
    } else {
        source_code.to_string()
    };

    // Inject `module.exports` for CommonJS-style default export.
    // Plugin writes: module.exports = { pluginId() {}, resolve(url) {}, ... }
    // After eval, __plugin_exports points to module.exports.
    let wrapped = format!(
        r#"var module = {{ exports: {{}} }};
{js_source}
var __plugin_exports = module.exports;
"#
    );

    if let Some(cache) = cache {
        match context.compile_to_bytecode(&wrapped, filename) {
            Ok(bytecode) => {
                // SAFETY: produced just now by this engine, from checked source.
                unsafe { context.eval_bytecode(&bytecode, filename) }?;
                cache.put(&cache.key(filename, source_code), &bytecode);
                return Ok(());
            }
            Err(e @ crate::Error::Timeout(_)) => return Err(e),
            Err(e) => debug!("Bytecode compile of {filename} failed ({e}); evaluating source"),
        }
    }

    context.eval_source(&wrapped, filename)
}

/// Read and validate a plugin's manifest exports after its module body ran.
fn read_manifest(
    context: &PluginContext,
    file_path: String,
) -> Result<(PluginMeta, Option<DomainAllowlist>), crate::Error> {
    // Extract required metadata — supports both properties and functions
    let id = context.get_export_string("id")?;
    let name = context.get_export_string("name")?;
    let version = context.get_export_string("version")?;
    let url_pattern = context.get_export_string("urlPattern")?;

    // Only a genuinely absent export gets the legacy fallback; a getter that
    // throws or times out is an error, never "assume v1".
    let api_version = if context.has_export("apiVersion")? {
        let v = context.get_export_string("apiVersion")?;
        v.trim().parse::<u32>().map_err(|_| {
            crate::Error::Execution(format!(
                "Plugin {id}: apiVersion must be a positive integer major version, got {v:?}"
            ))
        })?
    } else {
        warn!(
            "Plugin {id} does not declare apiVersion; assuming 1. \
                 Declare `apiVersion: {}` — this fallback will be removed.",
            crate::HOST_API_VERSION
        );
        1
    };
    if !crate::is_supported_api_version(api_version) {
        return Err(crate::Error::IncompatibleVersion {
            required: format!("host API {api_version}"),
            current: format!(
                "host API {} (supports {}..={})",
                crate::HOST_API_VERSION,
                crate::MIN_SUPPORTED_API_VERSION,
                crate::HOST_API_VERSION
            ),
        });
    }

    let permissions: PluginPermissions = match context.get_export_json("permissions")? {
        Some(json) => serde_json::from_str(&json).map_err(|e| {
            crate::Error::Execution(format!("Plugin {id}: invalid permissions: {e}"))
        })?,
        None => PluginPermissions::default(),
    };
    let allowlist = permissions
        .domains
        .as_deref()
        .map(DomainAllowlist::parse)
        .transpose()
        .map_err(|e| crate::Error::Execution(format!("Plugin {id}: {e}")))?;
    if allowlist.is_none() {
        warn!("Plugin {id} declares no permissions.domains — it may reach any public host");
    }

    // Optional metadata
    let description = context.get_export_string("description").ok();
    let author = context.get_export_string("author").ok();

    // Plugin type determines matching priority
    let plugin_type = context
        .get_export_string("pluginType")
        .ok()
        .and_then(|t| match t.as_str() {
            "multi-hoster" => Some(PluginType::MultiHoster),
            "hoster" => Some(PluginType::Hoster),
            "generic" => Some(PluginType::Generic),
            _ => None,
        })
        .unwrap_or_default();

    let meta = PluginMeta {
        id,
        name,
        version,
        url_pattern,
        file_path,
        enabled: true,
        description,
        author,
        plugin_type,
        api_version,
        permissions,
    };
    Ok((meta, allowlist))
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
