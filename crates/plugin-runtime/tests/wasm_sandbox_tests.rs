//! The plugin host with every plugin in a Wasm instance
//! (docs/specs/plugin-wasm-sandbox.md): a call that traps, times out or
//! exceeds its memory fails alone, the plugin recovers in a fresh instance,
//! and the host needs nothing but its own executable.

use std::path::Path;
use std::time::Duration;

use krake_plugin_runtime::Error;
use krake_plugin_runtime::bytecode_cache::BytecodeCache;
use krake_plugin_runtime::loader::PluginLoader;
use krake_plugin_runtime::sandbox::SandboxLimits;

/// A plugin whose `resolve(url)` misbehaves on request and otherwise returns
/// one download named after an MD5 from the host API (so a rebuilt instance
/// proves its host API is bound again).
fn write_plugin(root: &Path, id: &str, module_prelude: &str) {
    let dir = root.join(id);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("plugin.js"),
        format!(
            r#"{module_prelude}
module.exports = {{
    id: "{id}",
    name: "{id}",
    version: "1.0.0",
    apiVersion: 1,
    permissions: {{ domains: ["{id}.test"] }},
    urlPattern: "https?://{id}\\.test/.+",
    resolve(url) {{
        if (url.indexOf("recurse") >= 0) {{ var f = function () {{ return f() + 1; }}; f(); }}
        if (url.indexOf("spin") >= 0) {{ while (true) {{}} }}
        if (url.indexOf("hog") >= 0) {{ var a = []; while (true) {{ a.push(new Array(65536).fill("x" + a.length)); }} }}
        if (url.indexOf("busy") >= 0) {{ var t = Date.now(); while (Date.now() - t < 600) {{}} }}
        if (url.indexOf("throw") >= 0) {{ throw new Error("ordinary failure"); }}
        return {{ name: "x", downloads: [{{ url: url, filename: krake.md5("{id}"), filesize: null, chunks_supported: true, max_chunks: null, headers: null, cookies: null, wait_seconds: null, mirrors: [] }}] }};
    }},
}};
"#
        ),
    )
    .unwrap();
}

fn limits(exec_secs: u64) -> SandboxLimits {
    SandboxLimits {
        max_execution_secs: exec_secs,
        max_load_secs: 2,
        ..SandboxLimits::default()
    }
}

async fn filename(loader: &PluginLoader, id: &str, url: &str) -> Result<Option<String>, Error> {
    let pkg = loader.resolve(id, url).await?;
    Ok(pkg.downloads[0].filename.clone())
}

async fn failing(loader: &PluginLoader, id: &str) -> bool {
    loader.get_plugin_meta(id).await.unwrap().failing
}

// ── AC-03 ────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_trap_or_timeout_fails_one_call_and_the_plugin_recovers() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(tmp.path(), "wild", "");
    write_plugin(tmp.path(), "steady", "");
    let loader = PluginLoader::new(tmp.path().to_path_buf(), limits(1)).unwrap();
    assert_eq!(loader.discover().await.unwrap().len(), 2);
    let expected = filename(&loader, "wild", "https://wild.test/ok")
        .await
        .unwrap();
    // An MD5 in hex: the host API answered.
    assert_eq!(expected.as_deref().map(str::len), Some(32));
    let pid = loader.plugin_host_pid().await.expect("host is running");

    for (url, timeout) in [
        ("https://wild.test/recurse", false),
        ("https://wild.test/spin", true),
    ] {
        let err = filename(&loader, "wild", url).await.unwrap_err();
        assert_eq!(matches!(err, Error::Timeout(_)), timeout, "{url}: {err}");
        assert!(
            matches!(err, Error::Timeout(_) | Error::Execution(_)),
            "{url}: {err}"
        );
        assert!(failing(&loader, "wild").await, "{url}: not marked failing");
        assert!(!failing(&loader, "steady").await);
        assert_eq!(
            loader.plugin_host_pid().await,
            Some(pid),
            "{url}: host restarted"
        );

        // The next call runs in a fresh instance, with its host API bound.
        assert_eq!(
            filename(&loader, "wild", "https://wild.test/ok")
                .await
                .unwrap(),
            expected,
            "{url}"
        );
        assert!(
            !failing(&loader, "wild").await,
            "{url}: still marked failing"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_ordinary_plugin_error_does_not_mark_the_plugin_failing() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(tmp.path(), "wild", "");
    let loader = PluginLoader::new(tmp.path().to_path_buf(), limits(1)).unwrap();
    loader.discover().await.unwrap();
    let err = filename(&loader, "wild", "https://wild.test/throw")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("ordinary failure"), "{err}");
    assert!(!failing(&loader, "wild").await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn other_plugins_calls_complete_while_one_traps() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(tmp.path(), "wild", "");
    write_plugin(tmp.path(), "steady", "");
    let loader = PluginLoader::new(tmp.path().to_path_buf(), limits(2)).unwrap();
    loader.discover().await.unwrap();

    let (steady, wild) = tokio::join!(
        filename(&loader, "steady", "https://steady.test/busy"),
        async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            filename(&loader, "wild", "https://wild.test/recurse").await
        }
    );
    assert!(wild.is_err());
    assert!(
        steady.unwrap().is_some(),
        "the in-flight call must complete"
    );
    assert!(!failing(&loader, "steady").await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_trap_or_timeout_while_loading_leaves_the_plugin_unregistered() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(tmp.path(), "deep", "(function f() { return f() + 1; })();");
    write_plugin(tmp.path(), "stuck", "while (true) {}");
    write_plugin(tmp.path(), "steady", "");
    let loader = PluginLoader::new(tmp.path().to_path_buf(), limits(1)).unwrap();

    for id in ["deep", "stuck"] {
        let err = loader
            .load_plugin(&tmp.path().join(id).join("plugin.js"))
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::Timeout(_) | Error::Execution(_)),
            "{id}: {err}"
        );
        assert!(
            loader.get_plugin_meta(id).await.is_none(),
            "{id} registered"
        );
    }
    // discover() skips both and still loads the healthy one.
    let ids: Vec<String> = loader
        .discover()
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.id)
        .collect();
    assert_eq!(ids, ["steady"]);
}

// ── AC-04 ────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exceeding_the_memory_limit_fails_the_call_and_the_next_one_works() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(tmp.path(), "wild", "");
    let small = SandboxLimits {
        max_memory_bytes: 16 * 1024 * 1024,
        ..limits(10)
    };
    let loader = PluginLoader::new(tmp.path().to_path_buf(), small).unwrap();
    loader.discover().await.unwrap();

    let err = filename(&loader, "wild", "https://wild.test/hog")
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Execution(_)), "{err}");
    assert!(err.to_string().contains("memory limit"), "{err}");
    assert!(failing(&loader, "wild").await);

    assert!(
        filename(&loader, "wild", "https://wild.test/ok")
            .await
            .unwrap()
            .is_some()
    );
    assert!(!failing(&loader, "wild").await);
}

// ── AC-06 ────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bytecode_of_another_guest_build_is_never_read() {
    let tmp = tempfile::tempdir().unwrap();
    let plugins = tmp.path().join("plugins");
    let cache_dir = tmp.path().join("bytecode");
    write_plugin(&plugins, "wild", "");
    let entries = || {
        let mut names: Vec<String> = std::fs::read_dir(&cache_dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };

    // An entry written by "another guest build": same plugin, another
    // fingerprint, and a payload that is not bytecode at all.
    let other = BytecodeCache::open(&cache_dir, 16)
        .unwrap()
        .with_fingerprint_for_test("guest=some-other-build");
    let source = std::fs::read_to_string(plugins.join("wild/plugin.js")).unwrap();
    other.put(
        &other.key("plugin.js", &source),
        b"bytecode of another engine",
    );
    let stale = entries();
    assert_eq!(stale.len(), 1);

    let loader = PluginLoader::new(plugins.clone(), limits(5))
        .unwrap()
        .with_bytecode_cache(BytecodeCache::open(&cache_dir, 16).unwrap());
    assert_eq!(loader.discover().await.unwrap().len(), 1);
    assert!(
        filename(&loader, "wild", "https://wild.test/ok")
            .await
            .unwrap()
            .is_some(),
        "the plugin loaded from source"
    );
    // This build wrote its own entry under its own key; the stale one was
    // not touched (it leaves with the cache's size bound).
    let now = entries();
    assert_eq!(now.len(), 2, "{now:?}");
    assert!(now.contains(&stale[0]));
}

// ── AC-08 ────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_plugin_host_needs_only_its_own_executable() {
    let tmp = tempfile::tempdir().unwrap();
    let alone = tmp.path().join("bin");
    std::fs::create_dir_all(&alone).unwrap();
    let built = Path::new(env!("CARGO_BIN_EXE_krake-plugin-host"));
    let copy = alone.join(built.file_name().unwrap());
    std::fs::copy(built, &copy).unwrap();
    assert_eq!(std::fs::read_dir(&alone).unwrap().count(), 1);

    let plugins = tmp.path().join("plugins");
    write_plugin(&plugins, "wild", "");
    let loader = PluginLoader::new(plugins, limits(5))
        .unwrap()
        .with_plugin_host_binary(copy);
    assert_eq!(loader.discover().await.unwrap().len(), 1);
    assert!(
        filename(&loader, "wild", "https://wild.test/ok")
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        std::fs::read_dir(&alone).unwrap().count(),
        1,
        "the host wrote next to itself"
    );
}

// ── AC-01 ────────────────────────────────────────────────────────────────

/// The plugin runtime has no native QuickJS left: plugin JavaScript can only
/// run in the Wasm guest.
#[test]
fn the_plugin_runtime_does_not_depend_on_native_quickjs() {
    let output = std::process::Command::new(env!("CARGO"))
        .args([
            "tree",
            "-p",
            "krake-plugin-runtime",
            "--edges",
            "normal",
            "--prefix",
            "none",
            "--offline",
            "--locked",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo tree runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8_lossy(&output.stdout);
    assert!(tree.contains("wasmtime "), "unexpected tree:\n{tree}");
    let quickjs: Vec<&str> = tree
        .lines()
        .filter(|line| line.starts_with("rquickjs"))
        .collect();
    assert!(
        quickjs.is_empty(),
        "native QuickJS in the tree: {quickjs:?}"
    );
}
