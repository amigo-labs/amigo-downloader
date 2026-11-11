//! The Wasm executor: runs plugin JavaScript in QuickJS compiled to
//! WebAssembly (`crates/plugin-guest`), one instance per plugin.
//!
//! A memory-safety bug in QuickJS corrupts only the instance's linear
//! memory. The guest has no system interface: it can compute, read a clock
//! and call a function of the host-function table ([`HOST_FNS`]) — nothing
//! else. See docs/specs/plugin-wasm-guest.md.
//!
//! [`WasmEngine`] holds the module, precompiled by `build.rs`;
//! [`WasmContext`] is one plugin's instance and offers the operations of
//! [`PluginContext`](crate::engine::PluginContext). Limits per instance:
//!
//! - **memory**: the linear memory, which holds the QuickJS heap and the
//!   engine itself, may not grow past `EngineConfig::max_memory`;
//! - **time**: each call gets a wall-clock deadline (epoch interruption);
//! - **stack**: the Wasm stack limit of `wasm_config.rs`.
//!
//! A call that hits a limit or traps leaves QuickJS in an unknown state, so
//! the instance is unusable afterwards: every further call fails, and the
//! owner creates a new one.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use wasmtime::{Caller, Engine, Instance, Linker, Memory, Module, Store, Trap, TypedFunc};

use crate::engine::{
    EngineConfig, RESOLVE_POLL, TEST_HARNESS, TEST_RUNNER, TestResults, failed_run,
    parse_resolve_result, post_process_script, resolve_script, test_results,
};
use crate::host_fns::{HOST_FNS, HostArgs, HostDispatch, HostFn, HostResult, JS_SHIM, Obj, Ret};

include!("wasm_config.rs");

/// The guest module, precompiled for this target by `build.rs`.
static GUEST: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/plugin_guest.cwasm"));

/// Identity of the embedded guest build, for the bytecode cache key.
pub(crate) fn guest_fingerprint() -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(GUEST))
}

/// How often the epoch advances; the granularity of deadlines.
const TICK: Duration = Duration::from_millis(20);

/// Upper bound on one request, response or host-call frame.
const MAX_FRAME_BYTES: usize = 256 * 1024 * 1024;

const TAG_NONE: u8 = 0;
const TAG_SOME: u8 = 1;
const TAG_ERR: u8 = 2;

/// Guest operations (`crates/plugin-guest/src/lib.rs`).
#[derive(Clone, Copy)]
enum Op {
    Init = 0,
    Eval = 1,
    EvalString = 2,
    Compile = 3,
    EvalBytecode = 4,
    ExportString = 5,
    HasExport = 6,
    ExportJson = 7,
    RequireFn = 8,
    PendingJob = 9,
    Bind = 10,
    SelfTest = 255,
}

fn exec(message: impl Into<String>) -> crate::Error {
    crate::Error::Execution(message.into())
}

/// Holds the guest module and creates plugin instances from it.
pub struct WasmEngine {
    engine: Engine,
    module: Module,
    linker: Linker<HostState>,
    ticker: Arc<Ticker>,
}

/// Keeps the epoch ticker thread running. The engine and every instance
/// hold one, so deadlines keep working for instances that outlive the
/// engine; the thread stops when the last holder is gone.
struct Ticker(Arc<AtomicBool>);

impl Drop for Ticker {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

impl WasmEngine {
    /// Load the embedded guest module.
    pub fn new() -> Result<Self, crate::Error> {
        Self::from_precompiled(GUEST)
    }

    /// Load a precompiled guest module. Fails, without a panic, when the
    /// bytes were not precompiled by this `wasmtime` for this CPU.
    fn from_precompiled(precompiled: &[u8]) -> Result<Self, crate::Error> {
        let engine = Engine::new(&guest_engine_config())
            .map_err(|e| exec(format!("cannot start the Wasm engine: {e}")))?;
        // SAFETY: `Module::deserialize` trusts its input to be the output of
        // `Engine::precompile_module`. The only caller outside tests passes
        // `GUEST`, which `build.rs` produced with exactly that function and
        // which is part of this binary. wasmtime checks the artifact's
        // version, target and configuration header and returns an error for
        // bytes that are not its own, which is all the test relies on.
        let module = unsafe { Module::deserialize(&engine, precompiled) }.map_err(|e| {
            exec(format!(
                "the embedded plugin engine does not fit this build or CPU: {e}"
            ))
        })?;

        let mut linker = Linker::new(&engine);
        linker
            .func_wrap("krake", "host_call", host_call)
            .and_then(|l| l.func_wrap("env", "__rquickjs_host_now_us", now_us))
            .map_err(|e| exec(format!("cannot define the guest's imports: {e}")))?;

        let alive = Arc::new(AtomicBool::new(true));
        let ticker_alive = Arc::clone(&alive);
        let ticker_engine = engine.weak();
        std::thread::Builder::new()
            .name("plugin-epoch".into())
            .spawn(move || {
                while ticker_alive.load(Ordering::Relaxed) {
                    std::thread::sleep(TICK);
                    match ticker_engine.upgrade() {
                        Some(engine) => engine.increment_epoch(),
                        None => break,
                    }
                }
            })
            .map_err(|e| exec(format!("cannot start the deadline ticker: {e}")))?;

        Ok(Self {
            engine,
            module,
            linker,
            ticker: Arc::new(Ticker(alive)),
        })
    }

    /// Names of everything the guest module imports, as `module.name`.
    pub fn guest_imports(&self) -> Vec<String> {
        self.module
            .imports()
            .map(|i| format!("{}.{}", i.module(), i.name()))
            .collect()
    }

    /// The imports the executor defines; the guest may import nothing else.
    pub fn defined_imports() -> &'static [&'static str] {
        &["krake.host_call", "env.__rquickjs_host_now_us"]
    }

    /// Create an instance for one plugin, with the host API bound.
    /// `dispatch` executes the parent-routed host functions and carries the
    /// plugin's identity, which JS can neither read nor change.
    pub fn create_context(
        &self,
        config: &EngineConfig,
        dispatch: Arc<dyn HostDispatch>,
    ) -> Result<WasmContext, crate::Error> {
        let context = self.create_unbound_context(config, dispatch)?;
        context.bind_host_api()?;
        Ok(context)
    }

    /// Create an instance whose JavaScript has no host API yet: `krake` and
    /// `console` do not exist until [`WasmContext::bind_host_api`]. The
    /// plugin host evaluates a plugin's module body in this state, so a
    /// module body can only define exports.
    pub fn create_unbound_context(
        &self,
        config: &EngineConfig,
        dispatch: Arc<dyn HostDispatch>,
    ) -> Result<WasmContext, crate::Error> {
        let mut store = Store::new(
            &self.engine,
            HostState {
                dispatch,
                max_memory: config.max_memory,
                memory_limit_hit: false,
            },
        );
        store.limiter(|state| state);
        // Instantiation runs no plugin code, but the deadline must be set
        // before any guest code runs.
        store.set_epoch_deadline(ticks(config.load_timeout));
        let instance = self
            .linker
            .instantiate(&mut store, &self.module)
            .map_err(|e| exec(format!("cannot instantiate the plugin engine: {e}")))?;
        let export = |e: wasmtime::Error| exec(format!("the plugin engine lacks an export: {e}"));
        let guest = Guest {
            memory: instance
                .get_memory(&mut store, "memory")
                .ok_or_else(|| exec("the plugin engine exports no memory"))?,
            alloc: instance
                .get_typed_func(&mut store, "alloc")
                .map_err(export)?,
            dealloc: instance
                .get_typed_func(&mut store, "dealloc")
                .map_err(export)?,
            call: instance
                .get_typed_func(&mut store, "call")
                .map_err(export)?,
            _instance: instance,
        };
        let context = WasmContext {
            inner: Mutex::new(Inner {
                store,
                guest,
                unusable: false,
            }),
            load_timeout: config.load_timeout,
            _ticker: Arc::clone(&self.ticker),
        };
        context.op(Op::Init, &[], config.load_timeout)?;
        Ok(context)
    }
}

/// The host-function table as the guest reads it: one function per line,
/// `object\tname\trequired\toptional\tret`. The guest calls by row index.
fn host_table() -> String {
    HOST_FNS
        .iter()
        .map(|f| {
            let object = match f.object {
                Obj::Krake => "krake",
                Obj::Console => "console",
            };
            let ret = match f.ret {
                Ret::Unit => "unit",
                Ret::Str => "str",
                Ret::OptStr => "optstr",
                Ret::Bool => "bool",
                Ret::OptNum => "optnum",
                Ret::StrArray => "strarray",
            };
            format!(
                "{object}\t{}\t{}\t{}\t{ret}\n",
                f.name, f.required, f.optional
            )
        })
        .collect()
}

/// Epoch ticks a call with `timeout` may run.
fn ticks(timeout: Duration) -> u64 {
    (timeout.as_millis() / TICK.as_millis()) as u64 + 1
}

/// Per-instance state the imports see.
struct HostState {
    dispatch: Arc<dyn HostDispatch>,
    max_memory: usize,
    /// Set when the guest asked for more memory than its limit.
    memory_limit_hit: bool,
}

impl wasmtime::ResourceLimiter for HostState {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        if desired > self.max_memory {
            self.memory_limit_hit = true;
            return Ok(false);
        }
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        // The guest's function table is fixed at link time; this only bounds
        // a guest that tries to grow it.
        Ok(desired <= 1 << 20)
    }
}

/// Import `env.__rquickjs_host_now_us`: the clock behind `Date.now()`.
fn now_us() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as f64)
        .unwrap_or(0.0)
}

/// Import `krake.host_call`: run host function `index` with the argument
/// frame at `ptr`/`len` and return the reply frame, allocated in the guest.
///
/// Everything the guest passes is untrusted: an index outside the table, a
/// frame outside its memory or a malformed frame fails the call with an
/// error, which traps the guest. Nothing outside the instance is touched.
fn host_call(
    mut caller: Caller<'_, HostState>,
    index: u32,
    ptr: u32,
    len: u32,
) -> wasmtime::Result<u64> {
    let function = HOST_FNS
        .get(index as usize)
        .ok_or_else(|| wasmtime::Error::msg(format!("host function index {index} out of range")))?;
    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| wasmtime::Error::msg("the guest exports no memory"))?;
    let frame = memory
        .data(&caller)
        .get(ptr as usize..)
        .and_then(|tail| tail.get(..len as usize))
        .ok_or_else(|| wasmtime::Error::msg("host call frame is outside the guest's memory"))?;
    let args = decode_host_args(function, frame).map_err(wasmtime::Error::msg)?;

    let result: HostResult = match function.route {
        crate::host_fns::Route::Local(call) => call(&args),
        crate::host_fns::Route::Parent => {
            let dispatch = Arc::clone(&caller.data().dispatch);
            dispatch.call(&function.qualified_name(), args)
        }
    };

    let reply = match result {
        Ok(None) => vec![TAG_NONE],
        Ok(Some(value)) => tagged(TAG_SOME, value.as_bytes()),
        Err(message) => tagged(TAG_ERR, message.as_bytes()),
    };
    if reply.len() > MAX_FRAME_BYTES {
        return Err(wasmtime::Error::msg("host function result is too large"));
    }
    let alloc: TypedFunc<u32, u32> = caller
        .get_export("alloc")
        .and_then(|e| e.into_func())
        .ok_or_else(|| wasmtime::Error::msg("the guest exports no alloc"))?
        .typed(&caller)?;
    let reply_ptr = alloc.call(&mut caller, reply.len() as u32)?;
    memory
        .write(&mut caller, reply_ptr as usize, &reply)
        .map_err(|_| wasmtime::Error::msg("the guest could not hold the host function result"))?;
    Ok((u64::from(reply_ptr) << 32) | reply.len() as u64)
}

fn tagged(tag: u8, bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 1);
    out.push(tag);
    out.extend_from_slice(bytes);
    out
}

/// Decode a host-call argument frame: `[nargs u8]` then per argument
/// `[present u8][len u32 LE][bytes]`. The shape must match the table row.
fn decode_host_args(function: &HostFn, frame: &[u8]) -> Result<HostArgs, String> {
    let malformed = || format!("malformed arguments for host function {}", function.name);
    let (&count, mut rest) = frame.split_first().ok_or_else(malformed)?;
    if count as usize != function.required + function.optional {
        return Err(malformed());
    }
    let mut args = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        let (&present, tail) = rest.split_first().ok_or_else(malformed)?;
        rest = tail;
        if present == 0 {
            if i < function.required {
                return Err(malformed());
            }
            args.push(None);
            continue;
        }
        let (len, tail) = rest.split_first_chunk::<4>().ok_or_else(malformed)?;
        let len = u32::from_le_bytes(*len) as usize;
        let bytes = tail.get(..len).ok_or_else(malformed)?;
        let text = std::str::from_utf8(bytes).map_err(|_| malformed())?;
        args.push(Some(text.to_string()));
        rest = &tail[len..];
    }
    if !rest.is_empty() {
        return Err(malformed());
    }
    Ok(args)
}

/// The guest's exports.
struct Guest {
    memory: Memory,
    alloc: TypedFunc<u32, u32>,
    dealloc: TypedFunc<(u32, u32), ()>,
    call: TypedFunc<(u32, u32), u64>,
    _instance: Instance,
}

struct Inner {
    store: Store<HostState>,
    guest: Guest,
    unusable: bool,
}

/// One plugin's instance of the guest: an isolated QuickJS runtime in its
/// own linear memory.
pub struct WasmContext {
    /// Serializes calls on this instance, like `PluginContext`'s exec lock.
    inner: Mutex<Inner>,
    load_timeout: Duration,
    _ticker: Arc<Ticker>,
}

impl WasmContext {
    /// The deadline used for module evaluation and export reads.
    pub fn load_timeout(&self) -> Duration {
        self.load_timeout
    }

    /// Install the host API (`krake.*`, `console.*`) in an instance created
    /// with [`WasmEngine::create_unbound_context`].
    pub fn bind_host_api(&self) -> Result<(), crate::Error> {
        self.op(
            Op::Bind,
            &[host_table().as_bytes(), JS_SHIM.as_bytes()],
            self.load_timeout,
        )
        .map(drop)
    }

    /// Whether an earlier call left this instance unusable (it hit a limit
    /// or trapped). The owner drops it and creates a new one.
    pub fn is_unusable(&self) -> bool {
        self.lock().unusable
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Run one guest operation under `timeout`.
    fn op(
        &self,
        op: Op,
        args: &[&[u8]],
        timeout: Duration,
    ) -> Result<Option<Vec<u8>>, crate::Error> {
        let mut inner = self.lock();
        if inner.unusable {
            return Err(exec(
                "the plugin instance is unusable after an earlier failure; reload the plugin",
            ));
        }
        let result = inner.run(op, args, timeout);
        if inner.store.data().memory_limit_hit {
            inner.unusable = true;
            return Err(exec(format!(
                "plugin exceeded its memory limit of {} bytes",
                inner.store.data().max_memory
            )));
        }
        result
    }

    fn op_text(
        &self,
        op: Op,
        args: &[&[u8]],
        timeout: Duration,
    ) -> Result<Option<String>, crate::Error> {
        self.op(op, args, timeout)?
            .map(|bytes| {
                String::from_utf8(bytes)
                    .map_err(|_| exec("the plugin engine returned invalid UTF-8"))
            })
            .transpose()
    }

    fn op_string(&self, op: Op, args: &[&[u8]], timeout: Duration) -> Result<String, crate::Error> {
        self.op_text(op, args, timeout)?
            .ok_or_else(|| exec("the plugin engine returned no value"))
    }

    /// Load and evaluate JavaScript source code under the load deadline.
    pub fn eval_source(&self, source: &str, filename: &str) -> Result<(), crate::Error> {
        self.eval_source_with_timeout(source, filename, self.load_timeout)
    }

    /// Load and evaluate JavaScript source code under an explicit deadline.
    pub fn eval_source_with_timeout(
        &self,
        source: &str,
        filename: &str,
        timeout: Duration,
    ) -> Result<(), crate::Error> {
        let what = format!("JS eval error in {filename}");
        self.op(Op::Eval, &[source.as_bytes(), what.as_bytes()], timeout)
            .map(drop)
    }

    /// Compile `source` as a global script and return its QuickJS bytecode,
    /// without executing it.
    pub fn compile_to_bytecode(
        &self,
        source: &str,
        filename: &str,
    ) -> Result<Vec<u8>, crate::Error> {
        self.op(
            Op::Compile,
            &[source.as_bytes(), filename.as_bytes()],
            self.load_timeout,
        )?
        .ok_or_else(|| exec("the plugin engine returned no bytecode"))
    }

    /// Execute bytecode produced by [`Self::compile_to_bytecode`], under the
    /// load deadline. QuickJS does not validate bytecode; unlike the native
    /// engine this needs no `unsafe`, because whatever a bad buffer does, it
    /// does inside the instance's memory and ends as an error here.
    pub fn eval_bytecode(&self, bytecode: &[u8], filename: &str) -> Result<(), crate::Error> {
        self.op(
            Op::EvalBytecode,
            &[bytecode, filename.as_bytes()],
            self.load_timeout,
        )
        .map(drop)
    }

    /// Read an export as a string; a zero-argument function is called.
    pub fn get_export_string(&self, name: &str) -> Result<String, crate::Error> {
        self.op_string(Op::ExportString, &[name.as_bytes()], self.load_timeout)
    }

    /// Whether an export is present (not `undefined` / `null`). A throwing
    /// or looping getter is an error, not "absent".
    pub fn has_export(&self, name: &str) -> Result<bool, crate::Error> {
        Ok(self.op_string(Op::HasExport, &[name.as_bytes()], self.load_timeout)? == "true")
    }

    /// Read an export as JSON, `None` when it is missing, `null` or
    /// `undefined`.
    pub fn get_export_json(&self, name: &str) -> Result<Option<String>, crate::Error> {
        let name_lit =
            serde_json::to_string(name).map_err(|e| exec(format!("bad export name: {e}")))?;
        self.op_text(Op::ExportJson, &[name_lit.as_bytes()], self.load_timeout)
            .map_err(|e| match e {
                // The guest quotes the literal; the native engine the name.
                crate::Error::Execution(m) => exec(m.replacen(
                    &format!("reading export {name_lit}"),
                    &format!("reading export '{name}'"),
                    1,
                )),
                other => other,
            })
    }

    /// Verify that an export exists and is a function.
    pub fn require_export_function(&self, name: &str) -> Result<(), crate::Error> {
        self.op(Op::RequireFn, &[name.as_bytes()], self.load_timeout)
            .map(drop)
    }

    /// Call `resolve(url)` and return its result as JSON. A returned Promise
    /// is settled by driving the job queue.
    pub fn call_resolve(&self, url: &str, timeout: Duration) -> Result<String, crate::Error> {
        let script = resolve_script(url)?;
        let deadline = Instant::now() + timeout;
        let what = b"resolve() eval failed";
        let mut result = self.op_string(Op::EvalString, &[script.as_bytes(), what], timeout)?;
        while result == "PENDING" {
            let Some(left) = deadline
                .checked_duration_since(Instant::now())
                .filter(|left| !left.is_zero())
            else {
                // Out of time between two steps: the Promise is abandoned
                // half-settled, so the instance is as unusable as after an
                // interrupt.
                self.lock().unusable = true;
                return Err(crate::Error::Timeout(timeout.as_secs().max(1)));
            };
            let has_jobs = self.op_string(Op::PendingJob, &[], left)? == "true";
            result = self.op_string(
                Op::EvalString,
                &[RESOLVE_POLL.as_bytes(), b"Job check failed"],
                left,
            )?;
            if result == "PENDING" && !has_jobs {
                return Err(exec("resolve() returned a Promise that never settled"));
            }
        }
        parse_resolve_result(&result)
    }

    /// Check if the plugin exports a postProcess function.
    pub fn has_post_process(&self) -> bool {
        let code = "typeof __plugin_exports !== 'undefined' && \
                    typeof __plugin_exports.postProcess === 'function' ? 'true' : 'false'";
        self.eval_js(code).is_ok_and(|v| v == "true")
    }

    /// Call the plugin's postProcess(context) and return the result as JSON.
    pub fn call_post_process(
        &self,
        context_json: &str,
        timeout: Duration,
    ) -> Result<String, crate::Error> {
        let script = post_process_script(context_json)?;
        self.op_string(
            Op::EvalString,
            &[script.as_bytes(), b"postProcess() failed"],
            timeout,
        )
    }

    /// Evaluate raw JavaScript under the load deadline and return the result
    /// as a string.
    pub fn eval_js(&self, code: &str) -> Result<String, crate::Error> {
        self.eval_js_with_timeout(code, self.load_timeout)
    }

    /// Evaluate raw JavaScript under an explicit deadline.
    pub fn eval_js_with_timeout(
        &self,
        code: &str,
        timeout: Duration,
    ) -> Result<String, crate::Error> {
        self.op_string(
            Op::EvalString,
            &[code.as_bytes(), b"JS eval error"],
            timeout,
        )
    }

    /// Make the guest misbehave on purpose (`trap`, `bad-index`,
    /// `bad-pointer`, `bad-frame`), for tests that prove a trapping or
    /// hostile guest is contained. Not reachable from plugin JavaScript.
    #[doc(hidden)]
    pub fn self_test(&self, kind: &str) -> Result<(), crate::Error> {
        self.op(Op::SelfTest, &[kind.as_bytes()], self.load_timeout)
            .map(drop)
    }

    /// Run a spec file against an already-loaded plugin. The harness and the
    /// spec body evaluate under the load deadline; running the tests gets
    /// `timeout`.
    pub fn run_tests(
        &self,
        spec_source: &str,
        spec_filename: &str,
        timeout: Duration,
    ) -> TestResults {
        if let Err(e) = self.eval_source(TEST_HARNESS, "<test-harness>") {
            return failed_run("<harness>", format!("Failed to inject test harness: {e}"));
        }
        if let Err(e) = self.eval_source(spec_source, spec_filename) {
            return failed_run("<spec>", format!("Spec file error: {e}"));
        }
        test_results(self.eval_js_with_timeout(TEST_RUNNER, timeout))
    }
}

impl Inner {
    /// Write the request, call the guest, read the response. Any failure
    /// between entering and leaving the guest makes the instance unusable.
    fn run(
        &mut self,
        op: Op,
        args: &[&[u8]],
        timeout: Duration,
    ) -> Result<Option<Vec<u8>>, crate::Error> {
        let mut request = vec![op as u8, args.len() as u8];
        for arg in args {
            request.extend_from_slice(&(arg.len() as u32).to_le_bytes());
            request.extend_from_slice(arg);
        }
        if request.len() > MAX_FRAME_BYTES {
            return Err(exec("request to the plugin engine is too large"));
        }

        self.store.data_mut().memory_limit_hit = false;
        self.store.set_epoch_deadline(ticks(timeout));
        let response = self.exchange(&request);
        let response = match response {
            Ok(response) => response,
            Err(e) => {
                self.unusable = true;
                return Err(classify(&e, timeout));
            }
        };

        let Some((&tag, body)) = response.split_first() else {
            self.unusable = true;
            return Err(exec("the plugin engine returned an empty response"));
        };
        match tag {
            TAG_NONE => Ok(None),
            TAG_SOME => Ok(Some(body.to_vec())),
            TAG_ERR => Err(exec(String::from_utf8_lossy(body))),
            _ => {
                self.unusable = true;
                Err(exec("the plugin engine returned a malformed response"))
            }
        }
    }

    fn exchange(&mut self, request: &[u8]) -> wasmtime::Result<Vec<u8>> {
        let Guest {
            memory,
            alloc,
            dealloc,
            call,
            ..
        } = &self.guest;
        let len = request.len() as u32;
        let ptr = alloc.call(&mut self.store, len)?;
        memory.write(&mut self.store, ptr as usize, request)?;
        let packed = call.call(&mut self.store, (ptr, len))?;
        dealloc.call(&mut self.store, (ptr, len))?;

        let (out_ptr, out_len) = ((packed >> 32) as u32, (packed & 0xffff_ffff) as u32);
        let response = memory
            .data(&self.store)
            .get(out_ptr as usize..)
            .and_then(|tail| tail.get(..out_len as usize))
            .ok_or_else(|| wasmtime::Error::msg("response is outside the guest's memory"))?
            .to_vec();
        dealloc.call(&mut self.store, (out_ptr, out_len))?;
        Ok(response)
    }
}

/// Map a failed guest call to `Timeout` (the deadline interrupted it) or
/// `Execution`.
fn classify(error: &wasmtime::Error, timeout: Duration) -> crate::Error {
    match error.downcast_ref::<Trap>() {
        Some(Trap::Interrupt) => crate::Error::Timeout(timeout.as_secs().max(1)),
        Some(Trap::StackOverflow) => exec("plugin exhausted its stack (unbounded recursion?)"),
        Some(trap) => exec(format!("the plugin engine trapped: {trap}")),
        None => exec(format!("the plugin engine failed: {}", error.root_cause())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foreign_bytes_are_refused_without_a_panic() {
        let err = match WasmEngine::from_precompiled(b"this is not a precompiled module") {
            Ok(_) => panic!("garbage must not load"),
            Err(e) => e.to_string(),
        };
        assert!(err.contains("does not fit this build or CPU"), "{err}");
    }

    #[test]
    fn host_call_arguments_must_match_the_table_row() {
        let get = HOST_FNS.iter().find(|f| f.name == "__rawHttpGet").unwrap();
        // url present, headers omitted
        let ok = [2, 1, 3, 0, 0, 0, b'a', b'b', b'c', 0];
        assert_eq!(
            decode_host_args(get, &ok).unwrap(),
            vec![Some("abc".to_string()), None]
        );
        // wrong count, missing required, length past the end, trailing bytes,
        // invalid UTF-8
        for bad in [
            &[1, 1, 0, 0, 0, 0][..],
            &[2, 0, 0][..],
            &[2, 1, 9, 0, 0, 0, b'a', 0][..],
            &[2, 1, 0, 0, 0, 0, 0, 7][..],
            &[2, 1, 1, 0, 0, 0, 0xff, 0][..],
            &[][..],
        ] {
            assert!(decode_host_args(get, bad).is_err(), "{bad:?}");
        }
    }
}
