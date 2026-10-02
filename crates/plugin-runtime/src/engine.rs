//! QuickJS-NG engine wrapper.
//!
//! Every plugin gets its own [`PluginContext`] backed by its **own**
//! `rquickjs::Runtime`. One runtime per plugin buys three things a shared
//! runtime cannot:
//!
//! - the memory limit is per plugin (64 MiB each, as documented) instead of a
//!   budget shared by every loaded plugin;
//! - the interrupt deadline is per plugin, so one plugin finishing can never
//!   disarm another plugin's timeout;
//! - plugins execute concurrently — rquickjs serializes all contexts of one
//!   runtime behind a single lock, so a shared runtime would serialize every
//!   plugin behind the slowest one.
//!
//! Every entry point that runs plugin JS — module evaluation, export reads
//! (which may hit getters), `resolve`, `postProcess`, spec runs — goes through
//! [`PluginContext::with_deadline`]. There is no way to execute untrusted
//! source without an armed deadline.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rquickjs::{Context, Ctx, FromJs, Function, Object, Runtime, Value};

/// Sentinel for "no active deadline".
const NO_DEADLINE: u64 = u64::MAX;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Engine configuration, applied to every runtime the engine creates.
pub struct EngineConfig {
    /// Max memory per plugin runtime.
    pub max_memory: usize,
    /// Max stack size per plugin runtime.
    pub max_stack_size: usize,
    /// Deadline for evaluating a plugin's module body and for reading its
    /// exports. A module body only defines exports, so this is much shorter
    /// than the `resolve()` budget.
    pub load_timeout: Duration,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_memory: 64 * 1024 * 1024, // 64MB
            max_stack_size: 1024 * 1024,  // 1MB
            load_timeout: Duration::from_secs(5),
        }
    }
}

/// Factory for isolated plugin contexts.
pub struct PluginEngine {
    config: EngineConfig,
}

impl PluginEngine {
    pub fn new(config: EngineConfig) -> Result<Self, crate::Error> {
        Ok(Self { config })
    }

    /// Create a new isolated context — with its own runtime, memory limit and
    /// interrupt deadline — for one plugin.
    pub fn create_context(&self) -> Result<PluginContext, crate::Error> {
        let runtime = Runtime::new().map_err(|e| {
            crate::Error::Execution(format!("Failed to create QuickJS runtime: {e}"))
        })?;

        runtime.set_memory_limit(self.config.max_memory);
        runtime.set_max_stack_size(self.config.max_stack_size);

        // Install an interrupt handler that aborts JS execution once the
        // active deadline expires. QuickJS polls it between bytecode
        // operations and from inside the regex engine (`lre_check_timeout`),
        // so both `while(true){}` and a catastrophically backtracking native
        // `RegExp` stop.
        let deadline_ms = Arc::new(AtomicU64::new(NO_DEADLINE));
        let reader = Arc::clone(&deadline_ms);
        runtime.set_interrupt_handler(Some(Box::new(move || {
            now_ms() > reader.load(Ordering::Relaxed)
        })));

        let context = Context::full(&runtime).map_err(|e| {
            crate::Error::Execution(format!("Failed to create QuickJS context: {e}"))
        })?;

        Ok(PluginContext {
            context,
            _runtime: runtime,
            deadline_ms,
            exec: Mutex::new(()),
            load_timeout: self.config.load_timeout,
        })
    }
}

/// An isolated QuickJS context (and runtime) for a single plugin.
pub struct PluginContext {
    context: Context,
    /// Kept alive for the context's lifetime; owns the interrupt handler.
    _runtime: Runtime,
    deadline_ms: Arc<AtomicU64>,
    /// Serializes executions on this context, so the deadline set by one call
    /// cannot be overwritten or cleared by a concurrent call on the same
    /// plugin.
    exec: Mutex<()>,
    load_timeout: Duration,
}

impl PluginContext {
    /// Run `f` with an active deadline. The interrupt handler aborts any JS
    /// that runs past `timeout`; the deadline is cleared on return. The
    /// closure also receives the absolute deadline (Unix epoch ms) so it can
    /// distinguish "eval raised because of interrupt" from "eval raised for
    /// other reasons" by comparing to `SystemTime::now`.
    fn with_deadline<F, R>(&self, timeout: Duration, f: F) -> R
    where
        F: FnOnce(Ctx<'_>, u64) -> R,
    {
        let _guard = self.exec.lock().unwrap_or_else(|e| e.into_inner());
        let deadline = now_ms().saturating_add(timeout.as_millis() as u64);
        self.deadline_ms.store(deadline, Ordering::Relaxed);
        let result = self.context.with(|ctx| f(ctx, deadline));
        self.deadline_ms.store(NO_DEADLINE, Ordering::Relaxed);
        result
    }

    /// The deadline used for module evaluation and export reads.
    pub fn load_timeout(&self) -> Duration {
        self.load_timeout
    }
}

/// Map a JS error to `Timeout` when the deadline has passed, else to
/// `Execution` with `what` as context and the thrown value's message.
fn js_error(
    ctx: &Ctx<'_>,
    e: rquickjs::Error,
    deadline_ms: u64,
    timeout: Duration,
    what: &str,
) -> crate::Error {
    classify_error(describe_js_error(ctx, e), deadline_ms, timeout, what)
}

/// `Timeout` when the deadline has passed (the error is the interrupt), else
/// `Execution` carrying `detail`.
fn classify_error(detail: String, deadline_ms: u64, timeout: Duration, what: &str) -> crate::Error {
    if now_ms() > deadline_ms {
        crate::Error::Timeout(timeout.as_secs().max(1))
    } else {
        crate::Error::Execution(format!("{what}: {detail}"))
    }
}

/// Render an rquickjs error. For a thrown JS value the generic
/// "Exception generated by QuickJS" is replaced by the pending exception's
/// own message (which also clears it from the context).
pub(crate) fn describe_js_error(ctx: &Ctx<'_>, e: rquickjs::Error) -> String {
    if !e.is_exception() {
        return e.to_string();
    }
    let thrown = ctx.catch();
    if let Some(ex) = thrown.as_exception() {
        match ex.message() {
            Some(msg) => msg,
            None => "unknown JS exception".to_string(),
        }
    } else if let Some(s) = thrown.as_string() {
        s.to_string()
            .unwrap_or_else(|_| "unprintable JS exception".into())
    } else {
        format!("{thrown:?}")
    }
}

impl PluginContext {
    /// Execute a block of code within this context, under the load deadline.
    /// Used for host-side setup (registering the host API) and in tests.
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(Ctx<'_>) -> R,
    {
        self.with_deadline(self.load_timeout, |ctx, _| f(ctx))
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
        self.with_deadline(timeout, |ctx, deadline| {
            ctx.eval::<Value<'_>, _>(source).map_err(|e| {
                js_error(
                    &ctx,
                    e,
                    deadline,
                    timeout,
                    &format!("JS eval error in {filename}"),
                )
            })?;
            Ok(())
        })
    }

    /// Compile `source` as a global script and return its QuickJS bytecode,
    /// without executing it. Only the local bytecode cache uses this; the
    /// bytecode is never shipped anywhere.
    pub fn compile_to_bytecode(
        &self,
        source: &str,
        filename: &str,
    ) -> Result<Vec<u8>, crate::Error> {
        let timeout = self.load_timeout;
        self.with_deadline(timeout, |ctx, deadline| {
            crate::bytecode_cache::compile_script(&ctx, source, filename).map_err(|e| {
                classify_error(
                    e,
                    deadline,
                    timeout,
                    &format!("JS compile error in {filename}"),
                )
            })
        })
    }

    /// Execute bytecode previously produced by [`Self::compile_to_bytecode`]
    /// on this machine, under the load deadline.
    ///
    /// # Safety
    /// QuickJS does not validate bytecode before executing it. `bytecode` must
    /// come from `compile_to_bytecode` of the same engine build — the bytecode
    /// cache guarantees this with an engine-versioned key and a checksum.
    pub unsafe fn eval_bytecode(
        &self,
        bytecode: &[u8],
        filename: &str,
    ) -> Result<(), crate::Error> {
        let timeout = self.load_timeout;
        self.with_deadline(timeout, |ctx, deadline| {
            // SAFETY: forwarded to the caller.
            unsafe { crate::bytecode_cache::eval_script_bytecode(&ctx, bytecode) }.map_err(|e| {
                classify_error(
                    e,
                    deadline,
                    timeout,
                    &format!("JS eval error in {filename}"),
                )
            })
        })
    }

    /// Read an export as a string — works for both properties and zero-arg functions.
    /// If the export is a function, it gets called. If it's a string, it's returned directly.
    pub fn get_export_string(&self, name: &str) -> Result<String, crate::Error> {
        let timeout = self.load_timeout;
        self.with_deadline(timeout, |ctx, deadline| {
            let global = ctx.globals();
            let exports: Object<'_> = global
                .get("__plugin_exports")
                .map_err(|e| crate::Error::Execution(format!("__plugin_exports not found: {e}")))?;
            let value: Value<'_> = exports.get(name).map_err(|e| {
                js_error(
                    &ctx,
                    e,
                    deadline,
                    timeout,
                    &format!("reading export '{name}'"),
                )
            })?;

            // If it's a function, call it; otherwise coerce to string
            if value.is_function() {
                let func = Function::from_js(&ctx, value).map_err(|e| {
                    crate::Error::Execution(format!("'{name}' is not callable: {e}"))
                })?;
                let result: String = func.call(()).map_err(|e| {
                    js_error(&ctx, e, deadline, timeout, &format!("{name}() failed"))
                })?;
                Ok(result)
            } else if value.is_string() {
                let s: String = String::from_js(&ctx, value).map_err(|e| {
                    crate::Error::Execution(format!("'{name}' is not a string: {e}"))
                })?;
                Ok(s)
            } else if value.is_undefined() || value.is_null() {
                Err(crate::Error::Execution(format!(
                    "Export '{name}' not found"
                )))
            } else {
                // Numbers, booleans etc. — coerce via JS String()
                let script = format!("String(__plugin_exports.{name})");
                let result: String = ctx.eval(script).map_err(|e| {
                    js_error(
                        &ctx,
                        e,
                        deadline,
                        timeout,
                        &format!("Could not coerce '{name}' to string"),
                    )
                })?;
                Ok(result)
            }
        })
    }

    /// Read an export as JSON (`JSON.stringify`), `None` when it is missing,
    /// `null` or `undefined`. Used for structured manifest fields such as
    /// `permissions`.
    pub fn get_export_json(&self, name: &str) -> Result<Option<String>, crate::Error> {
        let timeout = self.load_timeout;
        let name_lit = serde_json::to_string(name)
            .map_err(|e| crate::Error::Execution(format!("bad export name: {e}")))?;
        self.with_deadline(timeout, |ctx, deadline| {
            let script = format!(
                "(function() {{ var v = __plugin_exports[{name_lit}]; \
                 return v === undefined || v === null ? null : JSON.stringify(v); }})()"
            );
            let result: Option<String> = ctx.eval(script).map_err(|e| {
                js_error(
                    &ctx,
                    e,
                    deadline,
                    timeout,
                    &format!("reading export '{name}'"),
                )
            })?;
            Ok(result)
        })
    }

    /// Verify that an export exists and is a function. Returns an error if not.
    pub fn require_export_function(&self, name: &str) -> Result<(), crate::Error> {
        let timeout = self.load_timeout;
        self.with_deadline(timeout, |ctx, deadline| {
            let global = ctx.globals();
            let exports: Object<'_> = global
                .get("__plugin_exports")
                .map_err(|e| crate::Error::Execution(format!("__plugin_exports not found: {e}")))?;
            let value: Value<'_> = exports.get(name).map_err(|e| {
                js_error(
                    &ctx,
                    e,
                    deadline,
                    timeout,
                    &format!("reading export '{name}'"),
                )
            })?;
            if value.is_function() {
                Ok(())
            } else if value.is_undefined() || value.is_null() {
                Err(crate::Error::Execution(format!(
                    "missing required function '{name}'"
                )))
            } else {
                Err(crate::Error::Execution(format!(
                    "'{name}' must be a function, got {}",
                    value.type_name()
                )))
            }
        })
    }

    /// Call the async `resolve(url)` function and return the result as JSON string.
    /// The JS function returns a Promise which we resolve synchronously via the event loop.
    pub fn call_resolve(&self, url: &str, timeout: Duration) -> Result<String, crate::Error> {
        // Serialise the URL as a JSON string literal — also a valid JS string
        // literal — so quotes/backslashes/control chars are escaped correctly
        // instead of relying on hand-rolled string escaping.
        let url_lit = serde_json::to_string(url)
            .map_err(|e| crate::Error::Execution(format!("failed to encode url: {e}")))?;

        // We wrap the resolve call in a script that catches the Promise result
        let script = format!(
            r#"
            (function() {{
                var __resolve_result = null;
                var __resolve_error = null;
                var __resolve_done = false;

                var p = __plugin_exports.resolve({url_lit});
                if (p && typeof p.then === 'function') {{
                    p.then(function(r) {{
                        __resolve_result = JSON.stringify(r);
                        __resolve_done = true;
                    }}).catch(function(e) {{
                        __resolve_error = String(e);
                        __resolve_done = true;
                    }});
                }} else {{
                    // Synchronous return
                    __resolve_result = JSON.stringify(p);
                    __resolve_done = true;
                }}

                return __resolve_done ? (__resolve_error ? "ERROR:" + __resolve_error : __resolve_result) : "PENDING";
            }})()
            "#,
        );

        self.with_deadline(timeout, |ctx, deadline_ms| {
            let result: String = ctx
                .eval(script)
                .map_err(|e| js_error(&ctx, e, deadline_ms, timeout, "resolve() eval failed"))?;

            // If still pending, drive the event loop
            if result == "PENDING" {
                let deadline = std::time::Instant::now() + timeout;

                // Drive the QuickJS job queue until resolved
                loop {
                    if std::time::Instant::now() > deadline {
                        return Err(crate::Error::Timeout(timeout.as_secs().max(1)));
                    }

                    let has_jobs = ctx.execute_pending_job();

                    // Check if done
                    let check: String = ctx
                        .eval(r#"__resolve_done ? (__resolve_error ? "ERROR:" + __resolve_error : __resolve_result) : "PENDING""#)
                        .map_err(|e| js_error(&ctx, e, deadline_ms, timeout, "Job check failed"))?;

                    if check != "PENDING" {
                        return parse_resolve_result(&check);
                    }

                    if !has_jobs {
                        // No more jobs but still pending — something went wrong
                        return Err(crate::Error::Execution(
                            "resolve() returned a Promise that never settled".into(),
                        ));
                    }
                }
            }

            parse_resolve_result(&result)
        })
    }

    /// Check if the plugin exports a postProcess function.
    pub fn has_post_process(&self) -> bool {
        self.with_deadline(self.load_timeout, |ctx, _| {
            let global = ctx.globals();
            let exports: Result<Object<'_>, _> = global.get("__plugin_exports");
            match exports {
                Ok(obj) => {
                    let val: Result<Value<'_>, _> = obj.get("postProcess");
                    val.is_ok_and(|v| v.is_function())
                }
                Err(_) => false,
            }
        })
    }

    /// Call the plugin's postProcess(context) function and return the result as JSON.
    pub fn call_post_process(
        &self,
        context_json: &str,
        timeout: Duration,
    ) -> Result<String, crate::Error> {
        // Encode the JSON document as a JS string literal (valid JS, correctly
        // escaped) and parse it back inside the VM, instead of hand-rolling
        // quote escaping into a single-quoted literal.
        let ctx_lit = serde_json::to_string(context_json)
            .map_err(|e| crate::Error::Execution(format!("failed to encode context: {e}")))?;
        let script = format!(
            r#"
            (function() {{
                if (typeof __plugin_exports.postProcess !== 'function') {{
                    return JSON.stringify({{ success: true, message: "no postProcess hook" }});
                }}
                var ctx = JSON.parse({ctx_lit});
                var result = __plugin_exports.postProcess(ctx);
                return JSON.stringify(result);
            }})()
            "#,
        );

        // Enforce the timeout via the runtime-wide interrupt handler — this
        // catches synchronous infinite loops that a tokio-level timeout can't
        // see, since `Context::with` is a blocking call.
        self.with_deadline(timeout, |ctx, deadline| {
            let result: String = ctx
                .eval(script)
                .map_err(|e| js_error(&ctx, e, deadline, timeout, "postProcess() failed"))?;
            Ok(result)
        })
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
        self.with_deadline(timeout, |ctx, deadline| {
            let result: String = ctx
                .eval(code.to_string())
                .map_err(|e| js_error(&ctx, e, deadline, timeout, "JS eval error"))?;
            Ok(result)
        })
    }

    /// Run a spec file against an already-loaded plugin. The harness and the
    /// spec body evaluate under the load deadline; running the tests (which
    /// may call `resolve()` and make HTTP requests) gets `timeout`.
    /// Returns (passed, failed, results) where results contains per-test details.
    pub fn run_tests(
        &self,
        spec_source: &str,
        spec_filename: &str,
        timeout: Duration,
    ) -> TestResults {
        // Inject test harness, then run the spec
        let harness = r#"
var __tests = [];
var __test_results = [];

function test(name, fn) {
    __tests.push({ name: name, fn: fn });
}

function assert(condition, message) {
    if (!condition) {
        throw new Error(message || "Assertion failed");
    }
}

function assertEqual(actual, expected, message) {
    if (actual !== expected) {
        throw new Error(
            (message ? message + ": " : "") +
            "expected " + JSON.stringify(expected) +
            ", got " + JSON.stringify(actual)
        );
    }
}

function assertNotNull(value, message) {
    if (value === null || value === undefined) {
        throw new Error(message || "Expected non-null value");
    }
}

function skip(reason) {
    throw { __skip: true, reason: reason || "skipped" };
}
"#;

        let runner = r#"
for (var i = 0; i < __tests.length; i++) {
    var t = __tests[i];
    try {
        t.fn();
        __test_results.push({ name: t.name, passed: true, error: null, skipped: false });
    } catch (e) {
        if (e && e.__skip) {
            __test_results.push({ name: t.name, passed: true, error: null, skipped: true, skipReason: e.reason });
        } else {
            __test_results.push({ name: t.name, passed: false, error: String(e), skipped: false });
        }
    }
}
JSON.stringify(__test_results);
"#;

        // Inject harness
        if let Err(e) = self.eval_source(harness, "<test-harness>") {
            return TestResults {
                passed: 0,
                failed: 1,
                skipped: 0,
                results: vec![SingleTestResult {
                    name: "<harness>".into(),
                    passed: false,
                    skipped: false,
                    skip_reason: None,
                    error: Some(format!("Failed to inject test harness: {e}")),
                }],
            };
        }

        // Run spec file
        if let Err(e) = self.eval_source(spec_source, spec_filename) {
            return TestResults {
                passed: 0,
                failed: 1,
                skipped: 0,
                results: vec![SingleTestResult {
                    name: "<spec>".into(),
                    passed: false,
                    skipped: false,
                    skip_reason: None,
                    error: Some(format!("Spec file error: {e}")),
                }],
            };
        }

        // Execute tests and collect results
        match self.eval_js_with_timeout(runner, timeout) {
            Ok(json) => {
                let raw: Vec<serde_json::Value> = match serde_json::from_str(&json) {
                    Ok(v) => v,
                    Err(e) => {
                        return TestResults {
                            passed: 0,
                            failed: 1,
                            skipped: 0,
                            results: vec![SingleTestResult {
                                name: "<test-runner>".to_string(),
                                passed: false,
                                skipped: false,
                                skip_reason: None,
                                error: Some(format!("Test results JSON parse error: {e}")),
                            }],
                        };
                    }
                };
                let mut passed = 0;
                let mut failed = 0;
                let mut skipped = 0;
                let mut results = Vec::new();

                for entry in &raw {
                    let name = entry["name"].as_str().unwrap_or("?").to_string();
                    let ok = entry["passed"].as_bool().unwrap_or(false);
                    let is_skipped = entry["skipped"].as_bool().unwrap_or(false);
                    let error = entry["error"].as_str().map(|s| s.to_string());
                    let skip_reason = entry["skipReason"].as_str().map(|s| s.to_string());

                    if is_skipped {
                        skipped += 1;
                    } else if ok {
                        passed += 1;
                    } else {
                        failed += 1;
                    }
                    results.push(SingleTestResult {
                        name,
                        passed: ok,
                        skipped: is_skipped,
                        skip_reason,
                        error,
                    });
                }

                TestResults {
                    passed,
                    failed,
                    skipped,
                    results,
                }
            }
            Err(e) => TestResults {
                passed: 0,
                failed: 1,
                skipped: 0,
                results: vec![SingleTestResult {
                    name: "<runner>".into(),
                    passed: false,
                    skipped: false,
                    skip_reason: None,
                    error: Some(format!("Test runner failed: {e}")),
                }],
            },
        }
    }
}

/// Results from running a plugin spec file.
#[derive(Debug, Clone)]
pub struct TestResults {
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub results: Vec<SingleTestResult>,
}

#[derive(Debug, Clone)]
pub struct SingleTestResult {
    pub name: String,
    pub passed: bool,
    pub skipped: bool,
    pub skip_reason: Option<String>,
    pub error: Option<String>,
}

fn parse_resolve_result(result: &str) -> Result<String, crate::Error> {
    if let Some(err) = result.strip_prefix("ERROR:") {
        Err(crate::Error::Execution(err.to_string()))
    } else {
        Ok(result.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_create_context() {
        let engine = PluginEngine::new(EngineConfig::default()).unwrap();
        let ctx = engine.create_context().unwrap();

        ctx.with(|ctx| {
            let result: i32 = ctx.eval("1 + 2").unwrap();
            assert_eq!(result, 3);
        });
    }

    #[test]
    fn test_eval_source() {
        let engine = PluginEngine::new(EngineConfig::default()).unwrap();
        let ctx = engine.create_context().unwrap();

        ctx.eval_source(
            r#"
            var __plugin_exports = { id: "test", name: "Test Plugin" };
            "#,
            "test.js",
        )
        .unwrap();

        let id = ctx.get_export_string("id").unwrap();
        assert_eq!(id, "test");
    }

    #[test]
    fn test_eval_js() {
        let engine = PluginEngine::new(EngineConfig::default()).unwrap();
        let ctx = engine.create_context().unwrap();

        let result = ctx.eval_js(r#""hello" + " " + "world""#).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn post_process_timeout_interrupts_infinite_loop() {
        let engine = PluginEngine::new(EngineConfig::default()).unwrap();
        let ctx = engine.create_context().unwrap();

        // Define a postProcess that never returns.
        ctx.eval_source(
            r#"
            var __plugin_exports = {
                postProcess: function() { while (true) {} }
            };
            "#,
            "test.js",
        )
        .unwrap();

        let start = std::time::Instant::now();
        let err = ctx
            .call_post_process("{}", Duration::from_millis(250))
            .expect_err("postProcess with infinite loop must time out");
        let elapsed = start.elapsed();

        // Must terminate within a reasonable multiple of the timeout —
        // before the fix this hung indefinitely.
        assert!(
            elapsed < Duration::from_secs(5),
            "postProcess did not honour timeout; elapsed={elapsed:?}"
        );
        assert!(
            matches!(err, crate::Error::Timeout(_))
                || err.to_string().to_lowercase().contains("timeout")
                || err.to_string().to_lowercase().contains("interrupt"),
            "unexpected error: {err}"
        );
    }
}
