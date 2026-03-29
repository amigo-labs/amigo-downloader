//! QuickJS-NG engine wrapper.
//!
//! Manages the QuickJS runtime and provides isolated contexts for each plugin.

use std::time::Duration;

use rquickjs::{Context, Ctx, FromJs, Function, Object, Runtime, Value};

/// Engine configuration.
pub struct EngineConfig {
    /// Max memory across all plugin contexts.
    pub max_memory: usize,
    /// Max stack size per context.
    pub max_stack_size: usize,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_memory: 256 * 1024 * 1024, // 256MB
            max_stack_size: 1024 * 1024,   // 1MB
        }
    }
}

/// Central QuickJS-NG engine — one runtime, multiple contexts.
pub struct PluginEngine {
    runtime: Runtime,
}

impl PluginEngine {
    pub fn new(config: EngineConfig) -> Result<Self, crate::Error> {
        let runtime = Runtime::new().map_err(|e| {
            crate::Error::Execution(format!("Failed to create QuickJS runtime: {e}"))
        })?;

        runtime.set_memory_limit(config.max_memory);
        runtime.set_max_stack_size(config.max_stack_size);

        Ok(Self { runtime })
    }

    /// Create a new isolated context for a plugin.
    pub fn create_context(&self) -> Result<PluginContext, crate::Error> {
        let context = Context::full(&self.runtime).map_err(|e| {
            crate::Error::Execution(format!("Failed to create QuickJS context: {e}"))
        })?;

        Ok(PluginContext { context })
    }
}

/// An isolated QuickJS context for a single plugin.
pub struct PluginContext {
    context: Context,
}

impl PluginContext {
    /// Execute a block of code within this context.
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(Ctx<'_>) -> R,
    {
        self.context.with(f)
    }

    /// Load and evaluate JavaScript source code.
    pub fn eval_source(&self, source: &str, filename: &str) -> Result<(), crate::Error> {
        self.context.with(|ctx| {
            ctx.eval::<Value<'_>, _>(source).map_err(|e| {
                crate::Error::Execution(format!("JS eval error in {filename}: {e}"))
            })?;
            Ok(())
        })
    }

    /// Read an export as a string — works for both properties and zero-arg functions.
    /// If the export is a function, it gets called. If it's a string, it's returned directly.
    pub fn get_export_string(&self, name: &str) -> Result<String, crate::Error> {
        self.context.with(|ctx| {
            let global = ctx.globals();
            let exports: Object<'_> = global
                .get("__plugin_exports")
                .map_err(|e| crate::Error::Execution(format!("__plugin_exports not found: {e}")))?;
            let value: Value<'_> = exports
                .get(name)
                .map_err(|e| crate::Error::Execution(format!("Export '{name}' not found: {e}")))?;

            // If it's a function, call it; otherwise coerce to string
            if value.is_function() {
                let func = Function::from_js(&ctx, value).map_err(|e| {
                    crate::Error::Execution(format!("'{name}' is not callable: {e}"))
                })?;
                let result: String = func
                    .call(())
                    .map_err(|e| crate::Error::Execution(format!("{name}() failed: {e}")))?;
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
                    crate::Error::Execution(format!("Could not coerce '{name}' to string: {e}"))
                })?;
                Ok(result)
            }
        })
    }

    /// Verify that an export exists and is a function. Returns an error if not.
    pub fn require_export_function(&self, name: &str) -> Result<(), crate::Error> {
        self.context.with(|ctx| {
            let global = ctx.globals();
            let exports: Object<'_> = global
                .get("__plugin_exports")
                .map_err(|e| crate::Error::Execution(format!("__plugin_exports not found: {e}")))?;
            let value: Value<'_> = exports
                .get(name)
                .map_err(|e| crate::Error::Execution(format!("Export '{name}' not found: {e}")))?;
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
        let url = url.to_string();

        // We wrap the resolve call in a script that catches the Promise result
        let script = format!(
            r#"
            (function() {{
                var __resolve_result = null;
                var __resolve_error = null;
                var __resolve_done = false;

                var p = __plugin_exports.resolve("{url}");
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
            url = url.replace('\\', "\\\\").replace('"', "\\\""),
        );

        self.context.with(|ctx| {
            let result: String = ctx.eval(script).map_err(|e| {
                crate::Error::Execution(format!("resolve() eval failed: {e}"))
            })?;

            // If still pending, drive the event loop
            if result == "PENDING" {
                let deadline = std::time::Instant::now() + timeout;

                // Drive the QuickJS job queue until resolved
                loop {
                    if std::time::Instant::now() > deadline {
                        return Err(crate::Error::Timeout(timeout.as_secs()));
                    }

                    let has_jobs = ctx.execute_pending_job();

                    // Check if done
                    let check: String = ctx
                        .eval(r#"__resolve_done ? (__resolve_error ? "ERROR:" + __resolve_error : __resolve_result) : "PENDING""#)
                        .map_err(|e| crate::Error::Execution(format!("Job check failed: {e}")))?;

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
        self.context.with(|ctx| {
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
        let escaped = context_json.replace('\\', "\\\\").replace('\'', "\\'");
        let script = format!(
            r#"
            (function() {{
                if (typeof __plugin_exports.postProcess !== 'function') {{
                    return JSON.stringify({{ success: true, message: "no postProcess hook" }});
                }}
                var ctx = JSON.parse('{escaped}');
                var result = __plugin_exports.postProcess(ctx);
                return JSON.stringify(result);
            }})()
            "#,
        );

        self.context.with(|ctx| {
            let _deadline = std::time::Instant::now() + timeout;
            let result: String = ctx.eval(script).map_err(|e| {
                crate::Error::Execution(format!("postProcess() failed: {e}"))
            })?;
            Ok(result)
        })
    }

    /// Evaluate raw JavaScript and return the result as a string.
    pub fn eval_js(&self, code: &str) -> Result<String, crate::Error> {
        self.context.with(|ctx| {
            let result: String = ctx
                .eval(code.to_string())
                .map_err(|e| crate::Error::Execution(format!("JS eval error: {e}")))?;
            Ok(result)
        })
    }

    /// Run a spec file against an already-loaded plugin.
    /// Returns (passed, failed, results) where results contains per-test details.
    pub fn run_tests(&self, spec_source: &str, spec_filename: &str) -> TestResults {
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
        match self.eval_js(runner) {
            Ok(json) => {
                let raw: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap_or_default();
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
}
