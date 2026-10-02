//! Regression tests for the plugin-sandbox hardening (issues #78, #81, #82,
//! #83, #84): deadlines on every JS entry point, concurrent execution with
//! per-plugin deadlines, the per-plugin domain allowlist, host-API
//! versioning and the local bytecode cache.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use amigo_plugin_runtime::bytecode_cache::BytecodeCache;
use amigo_plugin_runtime::loader::PluginLoader;
use amigo_plugin_runtime::sandbox::SandboxLimits;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const RESULT: &str = r#"{ name: "x", downloads: [{ url: url, filename: null, filesize: null, chunks_supported: true, max_chunks: null, headers: null, cookies: null, wait_seconds: null, mirrors: [] }] }"#;

fn write_plugin(root: &Path, folder: &str, source: &str) -> std::path::PathBuf {
    let dir = root.join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("plugin.js");
    std::fs::write(&path, source).unwrap();
    path
}

fn plugin(id: &str, extra: &str, resolve_body: &str) -> String {
    format!(
        r#"module.exports = {{
    id: "{id}",
    name: "{id}",
    version: "1.0.0",
    apiVersion: 1,
    urlPattern: "https?://{id}\\.test/.+",
    {extra}
    resolve(url) {{ {resolve_body} }},
}};
"#
    )
}

fn limits(exec_secs: u64, load_secs: u64) -> SandboxLimits {
    SandboxLimits {
        max_execution_secs: exec_secs,
        max_load_secs: load_secs,
        ..SandboxLimits::default()
    }
}

// --- #81: deadline on every entry point ---------------------------------

#[tokio::test]
async fn infinite_loop_in_module_body_fails_to_load_within_budget() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(
        tmp.path(),
        "hang",
        &format!(
            "{}\nwhile (true) {{}}\n",
            plugin("hang", "", &format!("return {RESULT};"))
        ),
    );
    write_plugin(
        tmp.path(),
        "ok",
        &plugin("ok", "", &format!("return {RESULT};")),
    );

    let loader = PluginLoader::new(tmp.path().to_path_buf(), limits(30, 1)).unwrap();
    let start = Instant::now();
    let metas = loader.discover().await.unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(10),
        "discover() hung: {elapsed:?}"
    );
    assert!(
        metas.iter().all(|m| m.id != "hang"),
        "hanging plugin must not load"
    );
    assert!(
        metas.iter().any(|m| m.id == "ok"),
        "other plugins still load"
    );
}

#[tokio::test]
async fn looping_getter_on_manifest_export_is_interrupted() {
    let tmp = tempfile::tempdir().unwrap();
    let path = write_plugin(
        tmp.path(),
        "getter",
        &format!(
            r#"module.exports = {{
    get id() {{ while (true) {{}} }},
    name: "g", version: "1.0.0", urlPattern: ".*",
    resolve(url) {{ return {RESULT}; }},
}};"#
        ),
    );
    let loader = PluginLoader::new(tmp.path().to_path_buf(), limits(30, 1)).unwrap();
    let start = Instant::now();
    let err = loader.load_plugin(&path).await.expect_err("must not load");
    assert!(start.elapsed() < Duration::from_secs(10));
    assert!(
        matches!(err, amigo_plugin_runtime::Error::Timeout(_)),
        "expected timeout, got {err}"
    );
}

#[tokio::test]
async fn catastrophic_native_regexp_is_interrupted() {
    // `(a+)+$` against "aaaa…b" backtracks exponentially in libregexp. The
    // bundled quickjs-ng polls the interrupt handler from inside the regex
    // engine (`lre_check_timeout`), so the deadline still applies.
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(
        tmp.path(),
        "redos",
        &plugin(
            "redos",
            "",
            &format!(r#"/(a+)+$/.test("a".repeat(40) + "b"); return {RESULT};"#),
        ),
    );
    let loader = PluginLoader::new(tmp.path().to_path_buf(), limits(1, 5)).unwrap();
    loader.discover().await.unwrap();

    let start = Instant::now();
    let err = loader
        .resolve("redos", "https://redos.test/x")
        .await
        .expect_err("backtracking regex must be aborted");
    let elapsed = start.elapsed();
    assert!(
        matches!(err, amigo_plugin_runtime::Error::Timeout(_)),
        "expected timeout, got {err}"
    );
    assert!(
        elapsed < Duration::from_secs(10),
        "regex ran unbounded: {elapsed:?}"
    );
}

// --- #82: no global serialization, per-plugin deadlines --------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slow_plugin_does_not_block_other_plugins_or_metadata() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(tmp.path(), "slow", &plugin("slow", "", "while (true) {}"));
    write_plugin(
        tmp.path(),
        "fast",
        &plugin("fast", "", &format!("return {RESULT};")),
    );
    let loader = Arc::new(PluginLoader::new(tmp.path().to_path_buf(), limits(3, 5)).unwrap());
    loader.discover().await.unwrap();

    let slow = {
        let loader = Arc::clone(&loader);
        tokio::spawn(async move { loader.resolve("slow", "https://slow.test/x").await })
    };
    // Let the slow plugin get going.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let start = Instant::now();
    assert_eq!(loader.list_plugins().await.len(), 2);
    assert!(loader.match_url("https://fast.test/x").await.is_some());
    loader
        .resolve("fast", "https://fast.test/x")
        .await
        .expect("fast plugin resolves while slow one runs");
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "unrelated work waited on the slow plugin: {:?}",
        start.elapsed()
    );

    let err = slow.await.unwrap().expect_err("slow plugin times out");
    assert!(matches!(err, amigo_plugin_runtime::Error::Timeout(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_plugins_each_keep_their_own_deadline() {
    // Plugin "short" finishes (by timing out) after 1s; plugin "long" has the
    // same 1s budget but starts later. If finishing "short" cleared a shared
    // deadline, "long" would spin forever.
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(tmp.path(), "short", &plugin("short", "", "while (true) {}"));
    write_plugin(tmp.path(), "long", &plugin("long", "", "while (true) {}"));
    let loader = Arc::new(PluginLoader::new(tmp.path().to_path_buf(), limits(1, 5)).unwrap());
    loader.discover().await.unwrap();

    let a = {
        let loader = Arc::clone(&loader);
        tokio::spawn(async move { loader.resolve("short", "https://short.test/x").await })
    };
    tokio::time::sleep(Duration::from_millis(500)).await;
    let start = Instant::now();
    let b = {
        let loader = Arc::clone(&loader);
        tokio::spawn(async move { loader.resolve("long", "https://long.test/x").await })
    };

    let (ra, rb) = tokio::time::timeout(Duration::from_secs(15), async {
        (a.await.unwrap(), b.await.unwrap())
    })
    .await
    .expect("a plugin lost its deadline and ran unbounded");
    assert!(matches!(ra, Err(amigo_plugin_runtime::Error::Timeout(_))));
    assert!(matches!(rb, Err(amigo_plugin_runtime::Error::Timeout(_))));
    assert!(start.elapsed() < Duration::from_secs(5));
}

// --- #78: per-plugin domain allowlist --------------------------------------

/// Loopback is the only host wiremock listens on, so these tests enable
/// private-network access and use `127.0.0.1` vs `localhost` as two distinct
/// hosts that both reach the mock.
fn allowlist_limits() -> SandboxLimits {
    SandboxLimits {
        allow_private_network: true,
        ..SandboxLimits::default()
    }
}

fn fetching_plugin(id: &str, domains: &str) -> String {
    plugin(
        id,
        &format!("permissions: {{ domains: {domains} }},"),
        &format!(
            r#"var target = url.replace("https://{id}.test/", "");
               var r = amigo.httpGet(decodeURIComponent(target));
               return {{ name: String(r.status), downloads: [] }};"#
        ),
    )
}

async fn fetch(loader: &PluginLoader, id: &str, target: &str) -> Result<String, String> {
    loader
        .resolve(
            id,
            &format!("https://{id}.test/{}", urlencoding_encode(target)),
        )
        .await
        .map(|p| p.name)
        .map_err(|e| e.to_string())
}

fn urlencoding_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn domain_allowlist_is_enforced_per_plugin_and_per_hop() {
    let mock = MockServer::start().await;
    let port = mock.address().port();
    Mock::given(method("GET"))
        .and(path("/ok"))
        .respond_with(ResponseTemplate::new(200).set_body_string("hi"))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/hop"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("Location", format!("http://localhost:{port}/ok").as_str()),
        )
        .mount(&mock)
        .await;

    let tmp = tempfile::tempdir().unwrap();
    write_plugin(tmp.path(), "ip", &fetching_plugin("ip", r#"["127.0.0.1"]"#));
    write_plugin(
        tmp.path(),
        "named",
        &fetching_plugin("named", r#"["localhost"]"#),
    );
    write_plugin(tmp.path(), "open", &fetching_plugin("open", "undefined"));
    let loader = PluginLoader::new(tmp.path().to_path_buf(), allowlist_limits()).unwrap();
    let metas = loader.discover().await.unwrap();
    assert_eq!(metas.len(), 3, "{metas:?}");
    assert!(metas.iter().find(|m| m.id == "open").unwrap().is_unscoped());
    assert!(!metas.iter().find(|m| m.id == "ip").unwrap().is_unscoped());

    let by_ip = format!("http://127.0.0.1:{port}/ok");
    let by_name = format!("http://localhost:{port}/ok");
    let hop = format!("http://127.0.0.1:{port}/hop");

    // Allowed host passes; a non-declared host is rejected.
    assert_eq!(fetch(&loader, "ip", &by_ip).await.unwrap(), "200");
    let err = fetch(&loader, "ip", &by_name).await.unwrap_err();
    assert!(err.contains("permissions.domains"), "{err}");

    // Two plugins with different lists do not share enforcement.
    assert_eq!(fetch(&loader, "named", &by_name).await.unwrap(), "200");
    assert!(fetch(&loader, "named", &by_ip).await.is_err());

    // A redirect from an allowed host to a non-declared one is rejected.
    let err = fetch(&loader, "ip", &hop).await.unwrap_err();
    assert!(err.contains("permissions.domains"), "{err}");

    // An unscoped plugin keeps today's behaviour.
    assert_eq!(fetch(&loader, "open", &by_name).await.unwrap(), "200");
}

#[tokio::test]
async fn invalid_domain_entries_refuse_to_load() {
    let tmp = tempfile::tempdir().unwrap();
    let path = write_plugin(tmp.path(), "bad", &fetching_plugin("bad", r#"["*"]"#));
    let loader = PluginLoader::new(tmp.path().to_path_buf(), SandboxLimits::default()).unwrap();
    let err = loader.load_plugin(&path).await.unwrap_err().to_string();
    assert!(err.contains("bare"), "{err}");
}

#[tokio::test]
async fn allowlist_cannot_be_changed_from_js() {
    // Mutating the exported permissions after load has no effect: the list
    // was compiled into the host bindings at register time.
    let mock = MockServer::start().await;
    let port = mock.address().port();
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(
        tmp.path(),
        "sneaky",
        &plugin(
            "sneaky",
            r#"permissions: { domains: ["127.0.0.1"] },"#,
            &format!(
                r#"module.exports.permissions.domains.push("localhost");
                   __plugin_exports.permissions = {{ domains: ["localhost"] }};
                   var r = amigo.httpGet("http://localhost:{port}/x");
                   return {{ name: String(r.status), downloads: [] }};"#
            ),
        ),
    );
    let loader = PluginLoader::new(tmp.path().to_path_buf(), allowlist_limits()).unwrap();
    loader.discover().await.unwrap();
    let err = loader
        .resolve("sneaky", "https://sneaky.test/x")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("permissions.domains"), "{err}");
}

// --- #84: host-API version -----------------------------------------------

#[tokio::test]
async fn unsupported_api_version_is_refused_naming_both_versions() {
    let tmp = tempfile::tempdir().unwrap();
    let src = plugin("future", "", &format!("return {RESULT};"))
        .replace("apiVersion: 1", "apiVersion: 99");
    let path = write_plugin(tmp.path(), "future", &src);
    let loader = PluginLoader::new(tmp.path().to_path_buf(), SandboxLimits::default()).unwrap();
    let err = loader.load_plugin(&path).await.unwrap_err();
    assert!(
        matches!(err, amigo_plugin_runtime::Error::IncompatibleVersion { .. }),
        "{err}"
    );
    let msg = err.to_string();
    assert!(msg.contains("99"), "{msg}");
    assert!(
        msg.contains(&amigo_plugin_runtime::HOST_API_VERSION.to_string()),
        "{msg}"
    );
}

#[tokio::test]
async fn missing_api_version_loads_as_v1() {
    let tmp = tempfile::tempdir().unwrap();
    let src = plugin("legacy", "", &format!("return {RESULT};")).replace("apiVersion: 1,", "");
    let path = write_plugin(tmp.path(), "legacy", &src);
    let loader = PluginLoader::new(tmp.path().to_path_buf(), SandboxLimits::default()).unwrap();
    let meta = loader.load_plugin(&path).await.unwrap();
    assert_eq!(meta.api_version, 1);
}

// --- #83: local bytecode cache -------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn second_load_comes_from_bytecode_cache_and_corruption_falls_back() {
    let tmp = tempfile::tempdir().unwrap();
    let plugins = tmp.path().join("plugins");
    let cache_dir = tmp.path().join("bytecode");
    // TypeScript, so a cache hit also skips SWC.
    let dir = plugins.join("cached");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("plugin.ts"),
        r#"const label: string = "from-ts";
module.exports = {
    id: "cached", name: "Cached", version: "1.0.0", apiVersion: 1,
    urlPattern: "https?://cached\\.test/.+",
    resolve(url: string) { return { name: label, downloads: [] }; },
};"#,
    )
    .unwrap();

    let new_loader = || {
        PluginLoader::new(plugins.clone(), SandboxLimits::default())
            .unwrap()
            .with_bytecode_cache(BytecodeCache::open(&cache_dir, 16).unwrap())
    };

    // Cold start: compiles and populates the cache.
    let first = new_loader();
    first.discover().await.unwrap();
    let cache = BytecodeCache::open(&cache_dir, 16).unwrap();
    assert_eq!(cache.len(), 1, "first load must write one entry");
    assert_eq!(
        first
            .resolve("cached", "https://cached.test/a")
            .await
            .unwrap()
            .name,
        "from-ts"
    );

    // Warm start: same behaviour, served from bytecode.
    let second = new_loader();
    second.discover().await.unwrap();
    assert_eq!(
        second
            .resolve("cached", "https://cached.test/a")
            .await
            .unwrap()
            .name,
        "from-ts"
    );

    // Corrupt every entry: the loader must fall back to source and rewrite it.
    for entry in std::fs::read_dir(&cache_dir).unwrap().flatten() {
        let data = std::fs::read(entry.path()).unwrap();
        std::fs::write(entry.path(), &data[..data.len() / 2]).unwrap();
    }
    let third = new_loader();
    third.discover().await.unwrap();
    assert_eq!(
        third
            .resolve("cached", "https://cached.test/a")
            .await
            .unwrap()
            .name,
        "from-ts"
    );
    assert_eq!(cache.len(), 1, "corrupted entry must be replaced");
}
