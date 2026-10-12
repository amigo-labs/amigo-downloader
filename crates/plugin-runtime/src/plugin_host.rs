//! The plugin host: the child process that runs plugin JavaScript.
//!
//! The `amigo-plugin-host` binary calls [`run`]. It owns every QuickJS
//! runtime; the daemon owns none. It reads [`ToChild`] frames from stdin and
//! writes [`ToParent`] frames to stdout (see [`crate::ipc`]).
//!
//! The child does no I/O of its own: the daemon sends plugin and spec
//! sources and bytecode-cache entries, and every stateful host function
//! (HTTP, cookies, storage, logging, notifications, captcha) is a
//! [`ToParent::HostCall`] answered by the daemon. Pure helpers (regex,
//! crypto, URL, HTML) run here, so untrusted HTML is also parsed here.
//!
//! The child is fully synchronous: a reader loop on the main thread and one
//! thread per in-flight request. Each plugin context keeps its own exec lock
//! and deadline (see [`crate::engine`]), so plugins run concurrently. It exits
//! when stdin closes, i.e. when the daemon goes away.

use std::collections::HashMap;
use std::io::{self, BufReader, BufWriter, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::engine::{EngineConfig, PluginContext, PluginEngine};
use crate::host_api::{base64_decode_bytes, base64_encode_bytes};
use crate::host_fns::{HostArgs, HostDispatch, HostResult, register_host_api};
use crate::ipc::{self, Handle, Payload, Request, ToChild, ToParent};
use crate::permissions::DomainAllowlist;
use crate::sandbox::SandboxLimits;
use crate::types::{PluginMeta, PluginPermissions, PluginType};

/// Run the plugin host on stdin/stdout until stdin closes. Returns the
/// process exit code.
pub fn run() -> i32 {
    let host = Arc::new(Host {
        out: Mutex::new(BufWriter::new(io::stdout())),
        pending_calls: Mutex::new(HashMap::new()),
        next_call: AtomicU64::new(1),
        contexts: Mutex::new(HashMap::new()),
        next_handle: AtomicU64::new(1),
    });

    if host
        .send(&ToParent::Hello {
            protocol: ipc::PROTOCOL_VERSION,
        })
        .is_err()
    {
        return 1;
    }

    let mut stdin = BufReader::new(io::stdin());
    loop {
        let msg: ToChild = match ipc::read_frame(&mut stdin) {
            Ok(Some(msg)) => msg,
            // The daemon closed our stdin: it is shutting down or gone.
            Ok(None) => return 0,
            Err(e) => {
                eprintln!("amigo-plugin-host: bad frame from daemon: {e}");
                return 2;
            }
        };
        match msg {
            ToChild::Request { id, body } => {
                let worker = Arc::clone(&host);
                let spawned = std::thread::Builder::new()
                    .name(format!("plugin-req-{id}"))
                    .spawn(move || {
                        let result = worker.handle(id, body);
                        // A failed write means the daemon is gone; the reader
                        // loop sees EOF and exits.
                        let _ = worker.send(&ToParent::Response { id, result });
                    });
                if let Err(e) = spawned {
                    let _ = host.send(&ToParent::Response {
                        id,
                        result: Err(crate::Error::Execution(format!(
                            "plugin host could not start a worker: {e}"
                        ))),
                    });
                }
            }
            ToChild::HostReply { call_id, result } => {
                let waiter = host.lock_pending().remove(&call_id);
                if let Some(waiter) = waiter {
                    let _ = waiter.send(result);
                }
            }
            ToChild::Drop { handle } => {
                host.lock_contexts().remove(&handle);
            }
        }
    }
}

/// One loaded plugin context.
struct Loaded {
    context: PluginContext,
    /// Request currently executing on this context, stamped on host calls so
    /// the daemon can attribute them. The daemon serializes calls per plugin.
    request: Arc<AtomicU64>,
}

struct Host {
    out: Mutex<BufWriter<io::Stdout>>,
    pending_calls: Mutex<HashMap<u64, SyncSender<HostResult>>>,
    next_call: AtomicU64,
    contexts: Mutex<HashMap<Handle, Arc<Loaded>>>,
    next_handle: AtomicU64,
}

impl Host {
    fn send(&self, msg: &ToParent) -> io::Result<()> {
        let mut out = self.out.lock().unwrap_or_else(|e| e.into_inner());
        ipc::write_frame(&mut *out, msg)?;
        out.flush()
    }

    fn lock_pending(&self) -> std::sync::MutexGuard<'_, HashMap<u64, SyncSender<HostResult>>> {
        self.pending_calls.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn lock_contexts(&self) -> std::sync::MutexGuard<'_, HashMap<Handle, Arc<Loaded>>> {
        self.contexts.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn context(&self, handle: Handle) -> Result<Arc<Loaded>, crate::Error> {
        self.lock_contexts()
            .get(&handle)
            .cloned()
            .ok_or_else(|| crate::Error::NotFound(format!("plugin handle {handle}")))
    }

    fn handle(self: &Arc<Self>, id: u64, body: Request) -> Result<Payload, crate::Error> {
        match body {
            Request::Load {
                source,
                filename,
                is_typescript,
                cached_bytecode,
                want_bytecode,
                limits,
            } => self.load(
                LoadSpec {
                    source: &source,
                    filename: &filename,
                    is_typescript,
                    cached_bytecode: cached_bytecode.as_deref(),
                    want_bytecode,
                },
                &limits,
            ),
            Request::Resolve {
                handle,
                url,
                timeout_ms,
            } => {
                let loaded = self.context(handle)?;
                loaded.request.store(id, Ordering::Relaxed);
                let json = loaded
                    .context
                    .call_resolve(&url, Duration::from_millis(timeout_ms))?;
                Ok(Payload::Json { json: Some(json) })
            }
            Request::PostProcess {
                handle,
                context_json,
                timeout_ms,
            } => {
                let loaded = self.context(handle)?;
                loaded.request.store(id, Ordering::Relaxed);
                if !loaded.context.has_post_process() {
                    return Ok(Payload::Json { json: None });
                }
                let json = loaded
                    .context
                    .call_post_process(&context_json, Duration::from_millis(timeout_ms))?;
                Ok(Payload::Json { json: Some(json) })
            }
            Request::RunSpec {
                handle,
                spec_source,
                filename,
                is_typescript,
                timeout_ms,
            } => {
                let loaded = self.context(handle)?;
                loaded.request.store(id, Ordering::Relaxed);
                let spec_source = if is_typescript {
                    crate::transpiler::transpile(&spec_source, &filename)?
                } else {
                    spec_source
                };
                let results = loaded.context.run_tests(
                    &spec_source,
                    &filename,
                    Duration::from_millis(timeout_ms),
                );
                Ok(Payload::Spec { results })
            }
        }
    }

    fn load(
        self: &Arc<Self>,
        spec: LoadSpec<'_>,
        limits: &SandboxLimits,
    ) -> Result<Payload, crate::Error> {
        let engine = PluginEngine::new(EngineConfig {
            max_memory: usize::try_from(limits.max_memory_bytes).unwrap_or(usize::MAX),
            load_timeout: Duration::from_secs(limits.max_load_secs.max(1)),
            ..Default::default()
        })?;

        let mut context = engine.create_context()?;
        let mut new_bytecode = None;
        let mut cached_bytecode_rejected = false;
        let from_cache = match spec.cached_bytecode {
            Some(b64) => match eval_cached(&context, b64, spec.filename) {
                Ok(()) => true,
                Err(e @ crate::Error::Timeout(_)) => return Err(e),
                Err(_) => {
                    // Start over in a clean context; a genuinely broken
                    // module body fails again below and is reported as such.
                    cached_bytecode_rejected = true;
                    context = engine.create_context()?;
                    false
                }
            },
            None => false,
        };
        if !from_cache {
            let bytecode = eval_from_source(&context, &spec)?;
            new_bytecode = bytecode.map(|b| base64_encode_bytes(&b));
        }

        let mut warnings = Vec::new();
        let meta = read_manifest(&context, &mut warnings)?;
        let id = meta.id.clone();

        // Bind the host API after the module body ran (so the plugin's
        // declared identity is known to the daemon) and before `resolve()` or
        // any callback can run — the module body only defines exports and
        // never touches `amigo`. The daemon maps the handle to the plugin's
        // id, allowlist and request budget; JS can see neither.
        let handle = self.next_handle.fetch_add(1, Ordering::Relaxed);
        let request = Arc::new(AtomicU64::new(0));
        let dispatch = Arc::new(ParentDispatch {
            host: Arc::clone(self),
            handle,
            request: Arc::clone(&request),
        });
        context.with(|ctx| register_host_api(&ctx, dispatch))?;

        context
            .require_export_function("resolve")
            .map_err(|e| crate::Error::Execution(format!("Plugin {id}: {e}")))?;

        self.lock_contexts()
            .insert(handle, Arc::new(Loaded { context, request }));
        Ok(Payload::Loaded {
            handle,
            meta: Box::new(meta),
            new_bytecode,
            cached_bytecode_rejected,
            warnings,
        })
    }
}

struct LoadSpec<'a> {
    source: &'a str,
    filename: &'a str,
    is_typescript: bool,
    cached_bytecode: Option<&'a str>,
    want_bytecode: bool,
}

/// Forwards [`Route::Parent`](crate::host_fns::Route::Parent) calls of one
/// plugin context to the daemon and blocks until it answers.
struct ParentDispatch {
    host: Arc<Host>,
    handle: Handle,
    request: Arc<AtomicU64>,
}

impl HostDispatch for ParentDispatch {
    fn call(&self, name: &str, args: HostArgs) -> HostResult {
        let call_id = self.host.next_call.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = sync_channel(1);
        self.host.lock_pending().insert(call_id, tx);
        let sent = self.host.send(&ToParent::HostCall {
            call_id,
            request: self.request.load(Ordering::Relaxed),
            handle: self.handle,
            name: name.to_string(),
            args,
        });
        if let Err(e) = sent {
            self.host.lock_pending().remove(&call_id);
            return Err(format!("plugin host lost the daemon: {e}"));
        }
        rx.recv()
            .unwrap_or_else(|_| Err("plugin host is shutting down".into()))
    }
}

/// Evaluate a plugin's module body from cache bytecode sent by the daemon.
fn eval_cached(context: &PluginContext, b64: &str, filename: &str) -> Result<(), crate::Error> {
    let bytecode = base64_decode_bytes(b64).map_err(crate::Error::Execution)?;
    // SAFETY: the entry was produced by `compile_to_bytecode` under a cache
    // key that pins the engine version, and the daemon verified its checksum
    // before sending it. Malformed bytecode could still corrupt this process —
    // which is why it is only ever evaluated here, never in the daemon.
    unsafe { context.eval_bytecode(&bytecode, filename) }
}

/// Evaluate a plugin's module body from source: transpile (TS), wrap, compile
/// and run. Returns the compiled bytecode when the caller asked for it.
fn eval_from_source(
    context: &PluginContext,
    spec: &LoadSpec<'_>,
) -> Result<Option<Vec<u8>>, crate::Error> {
    let js_source = if spec.is_typescript {
        crate::transpiler::transpile(spec.source, spec.filename)?
    } else {
        spec.source.to_string()
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

    if spec.want_bytecode {
        match context.compile_to_bytecode(&wrapped, spec.filename) {
            Ok(bytecode) => {
                // SAFETY: produced just now by this engine, from checked source.
                unsafe { context.eval_bytecode(&bytecode, spec.filename) }?;
                return Ok(Some(bytecode));
            }
            Err(e @ crate::Error::Timeout(_)) => return Err(e),
            // Fall through: evaluating the source reports the real error.
            Err(_) => {}
        }
    }

    context.eval_source(&wrapped, spec.filename)?;
    Ok(None)
}

/// Read and validate a plugin's manifest exports after its module body ran.
/// `file_path` is left empty; the daemon owns paths. Non-fatal findings are
/// pushed to `warnings` for the daemon to log.
fn read_manifest(
    context: &PluginContext,
    warnings: &mut Vec<String>,
) -> Result<PluginMeta, crate::Error> {
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
        warnings.push(format!(
            "Plugin {id} does not declare apiVersion; assuming 1. \
                 Declare `apiVersion: {}` — this fallback will be removed.",
            crate::HOST_API_VERSION
        ));
        1
    };
    check_api_version(api_version)?;

    let permissions: PluginPermissions = match context.get_export_json("permissions")? {
        Some(json) => serde_json::from_str(&json).map_err(|e| {
            crate::Error::Execution(format!("Plugin {id}: invalid permissions: {e}"))
        })?,
        None => PluginPermissions::default(),
    };
    parse_allowlist(&id, &permissions)?;

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

    Ok(PluginMeta {
        id,
        name,
        version,
        url_pattern,
        file_path: String::new(),
        enabled: true,
        description,
        author,
        plugin_type,
        api_version,
        permissions,
        failing: false,
    })
}

/// Refuse a host-API major this runtime does not implement. Checked in the
/// child (for the error) and again in the daemon (which does not trust the
/// child's manifest).
pub(crate) fn check_api_version(api_version: u32) -> Result<(), crate::Error> {
    if crate::is_supported_api_version(api_version) {
        return Ok(());
    }
    Err(crate::Error::IncompatibleVersion {
        required: format!("host API {api_version}"),
        current: format!(
            "host API {} (supports {}..={})",
            crate::HOST_API_VERSION,
            crate::MIN_SUPPORTED_API_VERSION,
            crate::HOST_API_VERSION
        ),
    })
}

/// Compile a plugin's declared `permissions.domains`. `None` = unscoped.
pub(crate) fn parse_allowlist(
    id: &str,
    permissions: &PluginPermissions,
) -> Result<Option<DomainAllowlist>, crate::Error> {
    permissions
        .domains
        .as_deref()
        .map(DomainAllowlist::parse)
        .transpose()
        .map_err(|e| crate::Error::Execution(format!("Plugin {id}: {e}")))
}
