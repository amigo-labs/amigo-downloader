//! What every plugin engine instance shares: its configuration, the scripts
//! that call `resolve` / `postProcess` and run plugin specs, and the result
//! types.
//!
//! Plugin JavaScript itself runs in [`crate::wasm`]: QuickJS compiled to
//! WebAssembly, one instance per plugin. Every entry point that runs plugin
//! JS — module evaluation, export reads (which may hit getters), `resolve`,
//! `postProcess`, spec runs — goes through an operation of
//! [`WasmContext`](crate::wasm::WasmContext), and each of those arms a
//! deadline. There is no way to execute untrusted source without one.

use std::time::Duration;

/// Engine configuration, applied to every instance the engine creates.
pub struct EngineConfig {
    /// Max linear memory per plugin instance; it holds the QuickJS heap and
    /// the engine itself.
    pub max_memory: usize,
    /// Deadline for evaluating a plugin's module body and for reading its
    /// exports. A module body only defines exports, so this is much shorter
    /// than the `resolve()` budget.
    pub load_timeout: Duration,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_memory: 64 * 1024 * 1024, // 64MB
            load_timeout: Duration::from_secs(5),
        }
    }
}

/// Results from running a plugin spec file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TestResults {
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub results: Vec<SingleTestResult>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SingleTestResult {
    pub name: String,
    pub passed: bool,
    pub skipped: bool,
    pub skip_reason: Option<String>,
    pub error: Option<String>,
}

/// Script that calls `resolve(url)` and leaves its outcome in VM-global
/// state: it evaluates to the JSON result, `ERROR:<message>`, or `PENDING`
/// when `resolve` returned a Promise that has not settled yet.
pub(crate) fn resolve_script(url: &str) -> Result<String, crate::Error> {
    // Serialise the URL as a JSON string literal — also a valid JS string
    // literal — so quotes/backslashes/control chars are escaped correctly
    // instead of relying on hand-rolled string escaping.
    let url_lit = serde_json::to_string(url)
        .map_err(|e| crate::Error::Execution(format!("failed to encode url: {e}")))?;

    // We wrap the resolve call in a script that catches the Promise result
    Ok(format!(
        r#"
        (function() {{
            // On globalThis, not local to this function: RESOLVE_POLL reads
            // them from a later, separate evaluation.
            globalThis.__resolve_result = null;
            globalThis.__resolve_error = null;
            globalThis.__resolve_done = false;

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
    ))
}

/// Script that calls `postProcess(context)` and evaluates to its JSON result.
pub(crate) fn post_process_script(context_json: &str) -> Result<String, crate::Error> {
    // Encode the JSON document as a JS string literal (valid JS, correctly
    // escaped) and parse it back inside the VM, instead of hand-rolling
    // quote escaping into a single-quoted literal.
    let ctx_lit = serde_json::to_string(context_json)
        .map_err(|e| crate::Error::Execution(format!("failed to encode context: {e}")))?;
    Ok(format!(
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
    ))
}

/// Polls the state [`resolve_script`] leaves behind.
pub(crate) const RESOLVE_POLL: &str = r#"__resolve_done ? (__resolve_error ? "ERROR:" + __resolve_error : __resolve_result) : "PENDING""#;

/// Test harness injected before a plugin spec.
pub(crate) const TEST_HARNESS: &str = r#"
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

/// Runs the tests a spec registered and evaluates to their results as JSON.
pub(crate) const TEST_RUNNER: &str = r#"
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

/// A run that failed before any test ran.
pub(crate) fn failed_run(name: &str, error: String) -> TestResults {
    TestResults {
        passed: 0,
        failed: 1,
        skipped: 0,
        results: vec![SingleTestResult {
            name: name.into(),
            passed: false,
            skipped: false,
            skip_reason: None,
            error: Some(error),
        }],
    }
}

/// Turn the outcome of [`TEST_RUNNER`] into [`TestResults`].
pub(crate) fn test_results(run: Result<String, crate::Error>) -> TestResults {
    let json = match run {
        Ok(json) => json,
        Err(e) => return failed_run("<runner>", format!("Test runner failed: {e}")),
    };
    let raw: Vec<serde_json::Value> = match serde_json::from_str(&json) {
        Ok(v) => v,
        Err(e) => {
            return failed_run(
                "<test-runner>",
                format!("Test results JSON parse error: {e}"),
            );
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

pub(crate) fn parse_resolve_result(result: &str) -> Result<String, crate::Error> {
    if let Some(err) = result.strip_prefix("ERROR:") {
        Err(crate::Error::Execution(err.to_string()))
    } else {
        Ok(result.to_string())
    }
}
