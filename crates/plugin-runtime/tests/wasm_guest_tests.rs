//! The Wasm executor against the recorded answers of the native engine and
//! against its limits.
//! docs/specs/plugin-wasm-guest.md

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use krake_plugin_runtime::Error;
use krake_plugin_runtime::engine::{EngineConfig, TestResults};
use krake_plugin_runtime::host_fns::{HostArgs, HostDispatch, HostResult};
use krake_plugin_runtime::transpiler::transpile;
use krake_plugin_runtime::wasm::{WasmContext, WasmEngine};

const PAGE: &str = r#"<html><head><title>My File</title></head><body><a class="dl" href="/get/file.zip">download</a></body></html>"#;

/// Parent side of the host API: records every call and answers HTTP and
/// storage from memory.
#[derive(Default)]
struct Recorder {
    calls: Mutex<Vec<(String, HostArgs)>>,
    storage: Mutex<HashMap<String, String>>,
}

impl Recorder {
    fn calls(&self) -> Vec<(String, HostArgs)> {
        self.calls.lock().unwrap().clone()
    }
}

impl HostDispatch for Recorder {
    fn call(&self, name: &str, args: HostArgs) -> HostResult {
        self.calls
            .lock()
            .unwrap()
            .push((name.to_string(), args.clone()));
        let arg = |i: usize| args.get(i).cloned().flatten().unwrap_or_default();
        match name {
            "__rawHttpGet" if arg(0).contains("fail") => Err("boom: network down".into()),
            "__rawHttpGet" | "__rawHttpPost" | "__rawHttpPostForm" => Ok(Some(
                serde_json::json!({ "status": 200, "headers": {}, "body": PAGE, "url": arg(0) })
                    .to_string(),
            )),
            "__rawHttpHead" => Ok(Some(
                serde_json::json!({ "status": 200, "headers": {}, "url": arg(0) }).to_string(),
            )),
            "storageSet" => {
                self.storage.lock().unwrap().insert(arg(0), arg(1));
                Ok(None)
            }
            "storageGet" => Ok(self.storage.lock().unwrap().get(&arg(0)).cloned()),
            "getCookie" => Ok(None),
            _ => Ok(None),
        }
    }
}

fn config(load_timeout: Duration) -> EngineConfig {
    EngineConfig {
        load_timeout,
        ..EngineConfig::default()
    }
}

fn engine() -> WasmEngine {
    WasmEngine::new().expect("the embedded guest loads")
}

fn wasm_with(engine: &WasmEngine, dispatch: Arc<dyn HostDispatch>) -> WasmContext {
    engine
        .create_context(&config(Duration::from_secs(5)), dispatch)
        .expect("instance")
}

/// What the native QuickJS engine answered before it left the plugin
/// runtime (docs/specs/plugin-wasm-sandbox.md, AC-02). Recorded from the
/// native engine at the last commit that had both, where the same tests
/// compared the two live.
static NATIVE_ANSWERS: &str = include_str!("fixtures/native_engine_expectations.json");

/// Replays the native engine's answers. They are keyed by the loaded source,
/// the operation, its argument and how often that call was made, so a test
/// drives this like an engine.
struct Native {
    answers: std::collections::BTreeMap<String, serde_json::Value>,
    state: Mutex<(String, HashMap<String, usize>)>,
}

impl Native {
    fn answer<T: serde::de::DeserializeOwned>(&self, op: &str, arg: &str) -> T {
        use sha2::Digest;
        let key = {
            let mut state = self.state.lock().unwrap();
            if op == "load" {
                state.0 = hex::encode(&sha2::Sha256::digest(arg.as_bytes())[..8]);
                state.1.clear();
            }
            let shown = if op == "load" { "" } else { arg };
            let call = format!("{}|{op}|{shown}", state.0);
            let n = state.1.entry(call.clone()).or_insert(0);
            *n += 1;
            format!("{call}|{n}")
        };
        let value = self
            .answers
            .get(&key)
            .unwrap_or_else(|| panic!("no recorded native answer for {key}"));
        serde_json::from_value(value.clone()).unwrap()
    }

    /// The parent-routed host calls the native engine made.
    fn calls(&self) -> Vec<(String, HostArgs)> {
        self.answer("calls", "")
    }
}

fn native() -> Native {
    Native {
        answers: serde_json::from_str(NATIVE_ANSWERS).expect("fixture is valid JSON"),
        state: Mutex::new((String::new(), HashMap::new())),
    }
}

/// The module wrapper the plugin host puts around a plugin's source.
fn wrap(js: &str) -> String {
    format!("var module = {{ exports: {{}} }};\n{js}\nvar __plugin_exports = module.exports;\n")
}

fn wrap_file(path: &Path) -> String {
    let source = std::fs::read_to_string(path).unwrap();
    let name = path.to_string_lossy();
    wrap(&transpile(&source, &name).unwrap())
}

fn spec_source(path: &Path) -> String {
    let source = std::fs::read_to_string(path).unwrap();
    transpile(&source, &path.to_string_lossy()).unwrap()
}

/// The operations both engines offer, so one function can drive either.
trait Js {
    fn load(&self, source: &str) -> Result<(), Error>;
    fn export_string(&self, name: &str) -> Result<String, Error>;
    fn export_json(&self, name: &str) -> Result<Option<String>, Error>;
    fn require_fn(&self, name: &str) -> Result<(), Error>;
    fn resolve(&self, url: &str) -> Result<String, Error>;
    fn post_process(&self, json: &str) -> Result<String, Error>;
    fn spec(&self, source: &str) -> TestResults;
    fn eval(&self, code: &str) -> Result<String, Error>;
}

macro_rules! impl_js {
    ($ty:ty) => {
        impl Js for $ty {
            fn load(&self, source: &str) -> Result<(), Error> {
                self.eval_source(source, "plugin.js")
            }
            fn export_string(&self, name: &str) -> Result<String, Error> {
                self.get_export_string(name)
            }
            fn export_json(&self, name: &str) -> Result<Option<String>, Error> {
                self.get_export_json(name)
            }
            fn require_fn(&self, name: &str) -> Result<(), Error> {
                self.require_export_function(name)
            }
            fn resolve(&self, url: &str) -> Result<String, Error> {
                self.call_resolve(url, Duration::from_secs(5))
            }
            fn post_process(&self, json: &str) -> Result<String, Error> {
                self.call_post_process(json, Duration::from_secs(5))
            }
            fn spec(&self, source: &str) -> TestResults {
                self.run_tests(source, "plugin.spec.js", Duration::from_secs(5))
            }
            fn eval(&self, code: &str) -> Result<String, Error> {
                self.eval_js(code)
            }
        }
    };
}
impl_js!(WasmContext);

impl Js for Native {
    fn load(&self, source: &str) -> Result<(), Error> {
        self.answer("load", source)
    }
    fn export_string(&self, name: &str) -> Result<String, Error> {
        self.answer("export_string", name)
    }
    fn export_json(&self, name: &str) -> Result<Option<String>, Error> {
        self.answer("export_json", name)
    }
    fn require_fn(&self, name: &str) -> Result<(), Error> {
        self.answer("require_fn", name)
    }
    fn resolve(&self, url: &str) -> Result<String, Error> {
        self.answer("resolve", url)
    }
    fn post_process(&self, json: &str) -> Result<String, Error> {
        self.answer("post_process", json)
    }
    fn spec(&self, _source: &str) -> TestResults {
        self.answer("spec", "")
    }
    fn eval(&self, code: &str) -> Result<String, Error> {
        self.answer("eval", code)
    }
}

fn text<T: std::fmt::Debug>(result: Result<T, Error>) -> String {
    match result {
        Ok(value) => format!("ok: {value:?}"),
        Err(e) => format!("err: {e}"),
    }
}

/// Everything the plugin host reads from a freshly loaded plugin.
fn manifest(js: &dyn Js, source: &str) -> Vec<String> {
    let mut out = vec![text(js.load(source))];
    for field in [
        "id",
        "name",
        "version",
        "apiVersion",
        "urlPattern",
        "pluginType",
    ] {
        out.push(format!("{field} = {}", text(js.export_string(field))));
    }
    out.push(format!(
        "permissions = {}",
        text(js.export_json("permissions"))
    ));
    out.push(format!("resolve = {}", text(js.require_fn("resolve"))));
    out
}

fn json(result: Result<String, Error>) -> serde_json::Value {
    serde_json::from_str(&result.expect("call succeeds")).expect("valid JSON")
}

fn shipped_plugins() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let mut found = Vec::new();
    for group in ["hosters", "multi-hosters", "extractors"] {
        for entry in std::fs::read_dir(root.join(group)).unwrap() {
            let plugin = entry.unwrap().path().join("plugin.ts");
            if plugin.is_file() {
                found.push(plugin);
            }
        }
    }
    found.push(root.join("template/plugin.ts"));
    found.sort();
    assert!(found.len() >= 7, "shipped plugins not found: {found:?}");
    found
}

const HELPER_PLUGIN: &str = r#"
module.exports = {
    id: "helper", name: "Helper", version: "1.2.3", apiVersion: 1,
    urlPattern: "https?://example\\.com/.+",
    pluginType: "hoster",
    permissions: { domains: ["example.com"] },
    resolve(url) {
        var page = krake.httpGet(url, { headers: { "X-Test": "1" } });
        var link = krake.htmlQueryAttr(page.body, "a.dl", "href");
        krake.storageSet("last", url);
        return {
            name: krake.htmlExtractTitle(page.body),
            id: krake.regexMatch("file\\.(\\w+)", link),
            hash: krake.sha256(url),
            stored: krake.storageGet("last"),
            parts: krake.regexMatchAll("\\w+", "a b c"),
            seconds: krake.parseDuration("1:30"),
            ok: krake.regexTest("^https", url),
            downloads: [{ url: krake.urlResolve(url, link) }],
        };
    },
    postProcess(ctx) {
        return { success: true, message: "saw " + ctx.files.length + " files" };
    },
};
"#;

// ── AC-01 ────────────────────────────────────────────────────────────────

#[test]
fn guest_imports_only_what_the_executor_defines() {
    let imports = engine().guest_imports();
    assert!(!imports.is_empty());
    for import in &imports {
        assert!(
            WasmEngine::defined_imports().contains(&import.as_str()),
            "the guest imports `{import}`, which the executor does not define"
        );
        assert!(!import.contains("wasi"), "WASI import: {import}");
    }
}

// ── AC-02 ────────────────────────────────────────────────────────────────

#[test]
fn manifests_of_shipped_plugins_match_the_native_engine() {
    let engine = engine();
    for path in shipped_plugins() {
        let source = wrap_file(&path);
        let dispatch = Arc::new(Recorder::default());
        let wasm = manifest(&wasm_with(&engine, dispatch.clone()), &source);
        let native = manifest(&native(), &source);
        assert_eq!(wasm, native, "{}", path.display());
        assert!(wasm[0].starts_with("ok"), "{}: {}", path.display(), wasm[0]);
    }
}

#[test]
fn invalid_sources_fail_with_the_native_message() {
    let engine = engine();
    let sources = [
        (
            "syntax error",
            "module.exports = { id: 'x', resolve(url) { return ; ".to_string(),
        ),
        (
            "no resolve",
            "module.exports = { id: 'x', name: 'X' };".to_string(),
        ),
        (
            "wrong types",
            "module.exports = { id: {}, name: 7, resolve: 'no' };".to_string(),
        ),
        (
            "throwing getter",
            "module.exports = { get id() { throw new Error('nope'); }, resolve() {} };".to_string(),
        ),
        ("empty", String::new()),
    ];
    for (label, source) in sources {
        let source = wrap(&source);
        let dispatch = Arc::new(Recorder::default());
        let wasm = manifest(&wasm_with(&engine, dispatch.clone()), &source);
        let native = manifest(&native(), &source);
        assert_eq!(wasm, native, "{label}");
        assert!(
            wasm.iter().any(|line| line.contains("err: ")),
            "{label}: {wasm:?}"
        );
    }
}

// ── AC-03 ────────────────────────────────────────────────────────────────

#[test]
fn resolve_makes_the_same_host_calls_and_returns_the_same_package() {
    let source = wrap(HELPER_PLUGIN);
    let url = "https://example.com/files/42";

    let wasm_calls = Arc::new(Recorder::default());
    let wasm = wasm_with(&engine(), wasm_calls.clone());
    wasm.load(&source).unwrap();
    let wasm_result = json(wasm.resolve(url));

    let native = native();
    native.load(&source).unwrap();
    let native_result = json(native.resolve(url));

    assert_eq!(wasm_result, native_result);
    assert_eq!(wasm_result["name"], "My File");
    assert_eq!(
        wasm_result["downloads"][0]["url"],
        "https://example.com/get/file.zip"
    );
    assert_eq!(wasm_calls.calls(), native.calls());
    let names: Vec<String> = wasm_calls.calls().into_iter().map(|c| c.0).collect();
    assert_eq!(names, ["__rawHttpGet", "storageSet", "storageGet"]);
}

// ── AC-04 ────────────────────────────────────────────────────────────────

#[test]
fn post_process_matches_the_native_engine() {
    let source = wrap(HELPER_PLUGIN);
    let context = r#"{"files":["a.rar","b.rar"],"dir":"/downloads"}"#;
    let wasm = wasm_with(&engine(), Arc::new(Recorder::default()));
    wasm.load(&source).unwrap();
    let native = native();
    native.load(&source).unwrap();
    assert!(wasm.has_post_process());
    let result = json(wasm.post_process(context));
    assert_eq!(result, json(native.post_process(context)));
    assert_eq!(result["message"], "saw 2 files");
}

#[test]
fn shipped_plugin_specs_give_the_native_results() {
    let engine = engine();
    let mut ran = 0;
    for plugin in shipped_plugins() {
        let spec = plugin.with_file_name("plugin.spec.ts");
        if !spec.is_file() {
            continue;
        }
        let source = wrap_file(&plugin);
        let spec = spec_source(&spec);
        let wasm = wasm_with(&engine, Arc::new(Recorder::default()));
        wasm.load(&source).unwrap();
        let native = native();
        native.load(&source).unwrap();
        let wasm_results = serde_json::to_value(wasm.spec(&spec)).unwrap();
        let native_results = serde_json::to_value(native.spec(&spec)).unwrap();
        assert_eq!(wasm_results, native_results, "{}", plugin.display());
        assert!(
            wasm_results["results"]
                .as_array()
                .is_some_and(|r| !r.is_empty())
        );
        ran += 1;
    }
    assert!(ran >= 3, "expected the shipped specs, ran {ran}");
}

// ── AC-05 ────────────────────────────────────────────────────────────────

#[test]
fn bytecode_round_trip_behaves_like_the_source() {
    let engine = engine();
    let source = wrap(HELPER_PLUGIN);
    let url = "https://example.com/files/42";

    let from_source = wasm_with(&engine, Arc::new(Recorder::default()));
    let expected_manifest = manifest(&from_source, &source);
    let expected = json(from_source.resolve(url));

    let compiler = wasm_with(&engine, Arc::new(Recorder::default()));
    let bytecode = compiler.compile_to_bytecode(&source, "plugin.js").unwrap();
    assert!(!bytecode.is_empty());

    let fresh = wasm_with(&engine, Arc::new(Recorder::default()));
    fresh.eval_bytecode(&bytecode, "plugin.js").unwrap();
    // `manifest` loads from source first; read the fields directly here.
    let mut from_bytecode = vec![expected_manifest[0].clone()];
    for field in [
        "id",
        "name",
        "version",
        "apiVersion",
        "urlPattern",
        "pluginType",
    ] {
        from_bytecode.push(format!("{field} = {}", text(fresh.export_string(field))));
    }
    from_bytecode.push(format!(
        "permissions = {}",
        text(fresh.export_json("permissions"))
    ));
    from_bytecode.push(format!("resolve = {}", text(fresh.require_fn("resolve"))));
    assert_eq!(from_bytecode, expected_manifest);
    assert!(expected_manifest.iter().any(|l| l.contains("hoster")));
    assert_eq!(json(fresh.resolve(url)), expected);
}

#[test]
fn corrupt_bytecode_is_an_error_not_a_crash() {
    let engine = engine();
    let good = wasm_with(&engine, Arc::new(Recorder::default()))
        .compile_to_bytecode(&wrap(HELPER_PLUGIN), "plugin.js")
        .unwrap();
    for bad in [&good[..good.len() / 2], &b"not bytecode"[..], &[][..]] {
        let fresh = wasm_with(&engine, Arc::new(Recorder::default()));
        let err = fresh.eval_bytecode(bad, "plugin.js").unwrap_err();
        assert!(matches!(err, Error::Execution(_)), "{err}");
    }
}

// ── AC-06 ────────────────────────────────────────────────────────────────

#[test]
fn host_function_errors_are_catchable_js_errors() {
    let source = wrap(
        r#"module.exports = { id: "e", resolve(url) {
            if (url.indexOf("caught") >= 0) {
                try { krake.httpGet("https://fail.example/"); }
                catch (e) { return { caught: e instanceof Error, message: e.message }; }
            }
            return krake.httpGet("https://fail.example/");
        } };"#,
    );
    let wasm = wasm_with(&engine(), Arc::new(Recorder::default()));
    wasm.load(&source).unwrap();
    let native = native();
    native.load(&source).unwrap();

    let caught = json(wasm.resolve("https://x/caught"));
    assert_eq!(caught, json(native.resolve("https://x/caught")));
    assert_eq!(caught["caught"], true);
    // rquickjs prefixes the message ("Error converting from js ..."); both
    // engines do, and the comparison above pins that they agree.
    assert!(
        caught["message"]
            .as_str()
            .is_some_and(|m| m.ends_with("boom: network down")),
        "{caught}"
    );

    let uncaught = wasm.resolve("https://x/plain").unwrap_err();
    assert!(matches!(uncaught, Error::Execution(_)));
    assert!(
        uncaught.to_string().contains("boom: network down"),
        "{uncaught}"
    );
    assert_eq!(
        uncaught.to_string(),
        native.resolve("https://x/plain").unwrap_err().to_string()
    );
    assert!(!wasm.is_unusable(), "a JS error leaves the instance usable");
}

#[test]
fn a_missing_return_value_reads_like_the_native_engine() {
    let source = wrap(r#"module.exports = { id: "u", resolve(url) { }, postProcess(ctx) { } };"#);
    let wasm = wasm_with(&engine(), Arc::new(Recorder::default()));
    wasm.load(&source).unwrap();
    let native = native();
    native.load(&source).unwrap();

    assert_eq!(
        text(wasm.resolve("https://x/")),
        text(native.resolve("https://x/"))
    );
    assert_eq!(
        text(wasm.post_process("{}")),
        text(native.post_process("{}"))
    );
    assert_eq!(
        text(wasm.eval_js("undefined")),
        text(native.eval("undefined"))
    );
    assert!(text(wasm.resolve("https://x/")).contains("'undefined'"));
    assert!(!wasm.is_unusable());
}

/// Edge case: a plugin throws something that is not an `Error`.
#[test]
fn thrown_non_errors_carry_their_string_form_under_both_engines() {
    let source = wrap(
        r#"module.exports = { id: "t", resolve(url) {
            if (url.indexOf("string") >= 0) throw "plain string";
            if (url.indexOf("object") >= 0) throw { code: 7 };
            if (url.indexOf("number") >= 0) throw 42;
            if (url.indexOf("array") >= 0) throw [1, 2];
            throw null;
        } };"#,
    );
    let wasm = wasm_with(&engine(), Arc::new(Recorder::default()));
    wasm.load(&source).unwrap();
    let native = native();
    native.load(&source).unwrap();

    for (kind, form) in [
        ("string", "plain string"),
        ("object", "[object Object]"),
        ("number", "42"),
        ("array", "1,2"),
        ("null", "null"),
    ] {
        let url = format!("https://x/{kind}");
        let message = text(wasm.resolve(&url));
        assert_eq!(message, text(native.resolve(&url)), "{kind}");
        assert!(
            message.ends_with(&format!("resolve() eval failed: {form}")),
            "{kind}: {message}"
        );
    }
}

// ── AC-07 ────────────────────────────────────────────────────────────────

fn assert_times_out(
    label: &str,
    timeout: Duration,
    run: impl FnOnce(&WasmContext) -> Result<(), Error>,
) {
    let context = engine()
        .create_context(&config(timeout), Arc::new(Recorder::default()))
        .unwrap();
    let start = Instant::now();
    let err = run(&context).expect_err(label);
    let elapsed = start.elapsed();
    assert!(matches!(err, Error::Timeout(_)), "{label}: {err}");
    assert!(
        elapsed >= timeout && elapsed < timeout + Duration::from_secs(1),
        "{label}: took {elapsed:?}"
    );
    assert!(context.is_unusable(), "{label}: instance still usable");
    let again = context.eval_js("'x'").unwrap_err();
    assert!(again.to_string().contains("unusable"), "{label}: {again}");
}

#[test]
fn runaway_code_ends_with_a_timeout() {
    let t = Duration::from_millis(300);
    assert_times_out("module body", t, |c| {
        c.eval_source("while (true) {}", "p.js")
    });
    assert_times_out("resolve", t, |c| {
        c.eval_source(
            &wrap("module.exports = { resolve() { while (true) {} } };"),
            "p.js",
        )?;
        c.call_resolve("https://x/", t).map(drop)
    });
    assert_times_out("manifest getter", t, |c| {
        c.eval_source(
            &wrap("module.exports = { get id() { while (true) {} } };"),
            "p.js",
        )?;
        c.get_export_string("id").map(drop)
    });
    assert_times_out("backtracking RegExp", t, |c| {
        c.eval_js("String(/^(a+)+$/.test('a'.repeat(40) + 'b'))")
            .map(drop)
    });
}

/// Answers `__rawHttpGet` only after `delay`.
struct Slow(Duration);

impl HostDispatch for Slow {
    fn call(&self, name: &str, _args: HostArgs) -> HostResult {
        if name == "__rawHttpGet" {
            std::thread::sleep(self.0);
            return Ok(Some(
                r#"{"status":200,"headers":{},"body":"late","url":""}"#.into(),
            ));
        }
        Ok(None)
    }
}

#[test]
fn a_deadline_that_passes_during_a_host_call_ends_the_call_on_return() {
    let timeout = Duration::from_millis(300);
    let blocked_for = Duration::from_millis(900);
    // The plugin even tries to swallow the failure.
    let code = "try { krake.httpGet('https://slow.example/'); } catch (e) {} 'finished'";
    let context = engine()
        .create_context(&config(timeout), Arc::new(Slow(blocked_for)))
        .unwrap();
    let start = Instant::now();
    let err = context.eval_js(code).unwrap_err();
    let elapsed = start.elapsed();
    assert!(matches!(err, Error::Timeout(_)), "{err}");
    assert!(
        elapsed >= blocked_for && elapsed < blocked_for + Duration::from_secs(1),
        "took {elapsed:?}"
    );
    assert!(context.is_unusable());
}

// ── AC-08, AC-09, AC-10 ──────────────────────────────────────────────────

#[test]
fn exceeding_the_memory_limit_fails_the_call_only() {
    let engine = engine();
    let small = EngineConfig {
        max_memory: 16 * 1024 * 1024,
        ..config(Duration::from_secs(10))
    };
    let hungry = engine
        .create_context(&small, Arc::new(Recorder::default()))
        .unwrap();
    let other = wasm_with(&engine, Arc::new(Recorder::default()));

    let err = hungry
        .eval_js("var a = []; while (true) { a.push(new Array(65536).fill('x' + a.length)); }")
        .unwrap_err();
    assert!(matches!(err, Error::Execution(_)), "{err}");
    assert!(err.to_string().contains("memory limit"), "{err}");
    assert!(hungry.is_unusable());
    assert_eq!(other.eval_js("String(1 + 2)").unwrap(), "3");
}

#[test]
fn unbounded_recursion_fails_the_call_only() {
    let engine = engine();
    let deep = wasm_with(&engine, Arc::new(Recorder::default()));
    let err = deep
        .eval_js("function f() { return f() + 1; } String(f())")
        .unwrap_err();
    assert!(matches!(err, Error::Execution(_)), "{err}");
    assert!(deep.is_unusable());
    let other = wasm_with(&engine, Arc::new(Recorder::default()));
    assert_eq!(other.eval_js("String(1 + 2)").unwrap(), "3");
}

#[test]
fn a_trapping_guest_is_contained() {
    let engine = engine();
    let source = wrap(HELPER_PLUGIN);
    let doomed = wasm_with(&engine, Arc::new(Recorder::default()));
    doomed.load(&source).unwrap();
    let err = doomed.self_test("trap").unwrap_err();
    assert!(matches!(err, Error::Execution(_)), "{err}");
    assert!(err.to_string().contains("trapped"), "{err}");
    assert!(doomed.is_unusable());

    let fresh = wasm_with(&engine, Arc::new(Recorder::default()));
    fresh.load(&source).unwrap();
    assert_eq!(
        json(fresh.resolve("https://example.com/files/1"))["name"],
        "My File"
    );
}

// ── AC-16 ────────────────────────────────────────────────────────────────

#[test]
fn hostile_host_calls_are_rejected() {
    let engine = engine();
    for kind in ["bad-index", "bad-pointer", "bad-frame"] {
        let dispatch = Arc::new(Recorder::default());
        let context = wasm_with(&engine, dispatch.clone());
        let err = context.self_test(kind).unwrap_err();
        assert!(matches!(err, Error::Execution(_)), "{kind}: {err}");
        assert!(
            !err.to_string().contains("accepted a hostile"),
            "{kind}: the executor ran the call"
        );
        assert!(
            dispatch.calls().is_empty(),
            "{kind}: reached the dispatcher"
        );
        assert!(context.is_unusable(), "{kind}");
    }
    assert_eq!(
        wasm_with(&engine, Arc::new(Recorder::default()))
            .eval_js("'alive'")
            .unwrap(),
        "alive"
    );
}

// ── AC-11 ────────────────────────────────────────────────────────────────

#[test]
fn instances_share_no_state_and_keep_their_own_deadline() {
    let engine = engine();
    let a = wasm_with(&engine, Arc::new(Recorder::default()));
    let b = wasm_with(&engine, Arc::new(Recorder::default()));
    a.eval_source("globalThis.secret = 'a-only';", "a.js")
        .unwrap();
    assert_eq!(a.eval_js("String(globalThis.secret)").unwrap(), "a-only");
    assert_eq!(
        b.eval_js("String(typeof globalThis.secret)").unwrap(),
        "undefined"
    );

    // `a` times out while `b` is still computing; `b` must keep its own,
    // longer deadline and finish.
    let busy = "var t = Date.now(); var n = 0; while (Date.now() - t < 900) { n++; } 'done'";
    std::thread::scope(|scope| {
        let looping =
            scope.spawn(|| a.eval_js_with_timeout("while (true) {}", Duration::from_millis(200)));
        let working = scope.spawn(|| b.eval_js_with_timeout(busy, Duration::from_secs(10)));
        assert!(matches!(looping.join().unwrap(), Err(Error::Timeout(_))));
        assert_eq!(working.join().unwrap().unwrap(), "done");
    });
}

/// The other direction: calls finishing on one instance must not clear the
/// deadline of another that is still running.
#[test]
fn a_finishing_instance_does_not_disarm_another_deadline() {
    let engine = engine();
    let looping = wasm_with(&engine, Arc::new(Recorder::default()));
    let quick = wasm_with(&engine, Arc::new(Recorder::default()));
    let timeout = Duration::from_millis(700);
    std::thread::scope(|scope| {
        let start = Instant::now();
        let stuck = scope.spawn(|| looping.eval_js_with_timeout("while (true) {}", timeout));
        let mut finished = 0;
        while start.elapsed() < Duration::from_millis(500) {
            assert_eq!(quick.eval_js("String(2 * 21)").unwrap(), "42");
            finished += 1;
        }
        assert!(finished >= 5, "only {finished} calls finished meanwhile");
        assert!(matches!(stuck.join().unwrap(), Err(Error::Timeout(_))));
        assert!(start.elapsed() < timeout + Duration::from_secs(1));
    });
}

// ── Edge case: resolve() returns a Promise ───────────────────────────────

#[test]
fn a_promise_from_resolve_is_settled_under_both_engines() {
    let source = wrap(
        r#"module.exports = { id: "p", resolve(url) {
            if (url.indexOf("reject") >= 0) {
                return Promise.reject(new Error("no such file"));
            }
            return Promise.resolve(url).then(function (u) {
                return { name: "from promise", downloads: [{ url: u }] };
            });
        } };"#,
    );
    let wasm = wasm_with(&engine(), Arc::new(Recorder::default()));
    wasm.load(&source).unwrap();
    let native = native();
    native.load(&source).unwrap();

    for js in [&wasm as &dyn Js, &native] {
        let ok = json(js.resolve("https://example.com/a"));
        assert_eq!(ok["name"], "from promise");
        assert_eq!(ok["downloads"][0]["url"], "https://example.com/a");
        let err = js.resolve("https://example.com/reject").unwrap_err();
        assert!(matches!(err, Error::Execution(_)), "{err}");
        assert!(err.to_string().contains("no such file"), "{err}");
        // A second call must not see the first one's state.
        assert_eq!(
            json(js.resolve("https://example.com/b"))["downloads"][0]["url"],
            "https://example.com/b"
        );
    }
}

// ── AC-12 ────────────────────────────────────────────────────────────────

/// Blocks `__rawHttpGet` until released.
struct Gate {
    entered: Mutex<Sender<()>>,
    release: Mutex<Receiver<()>>,
}

impl HostDispatch for Gate {
    fn call(&self, name: &str, _args: HostArgs) -> HostResult {
        if name == "__rawHttpGet" {
            self.entered.lock().unwrap().send(()).unwrap();
            self.release.lock().unwrap().recv().unwrap();
            return Ok(Some(
                r#"{"status":200,"headers":{},"body":"late","url":""}"#.into(),
            ));
        }
        Ok(None)
    }
}

#[test]
fn a_blocked_host_call_does_not_block_other_instances() {
    let engine = engine();
    let (entered_tx, entered_rx) = channel();
    let (release_tx, release_rx) = channel();
    let gate = Arc::new(Gate {
        entered: Mutex::new(entered_tx),
        release: Mutex::new(release_rx),
    });
    let blocked = wasm_with(&engine, gate);
    let free = wasm_with(&engine, Arc::new(Recorder::default()));

    std::thread::scope(|scope| {
        let waiting =
            scope.spawn(|| blocked.eval_js("krake.httpGet('https://slow.example/').body"));
        entered_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("host call started");
        assert_eq!(free.eval_js("String(6 * 7)").unwrap(), "42");
        release_tx.send(()).unwrap();
        assert_eq!(waiting.join().unwrap().unwrap(), "late");
    });
}
