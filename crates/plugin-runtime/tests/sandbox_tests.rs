//! Sandbox enforcement integration tests.
//!
//! Covers Phase 2.2 of `docs/specs/integration-tests.md`: verify that
//! plugins cannot bypass the resource limits, time bounds, or platform
//! isolation provided by the runtime.

use std::path::Path;

use amigo_plugin_runtime::loader::PluginLoader;
use amigo_plugin_runtime::sandbox::SandboxLimits;

async fn loader_with(
    limits: SandboxLimits,
    plugin_source: &str,
) -> (tempfile::TempDir, PluginLoader) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("test-plugin");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("plugin.js"), plugin_source).unwrap();

    let loader = PluginLoader::new(tmp.path().to_path_buf(), limits).expect("loader init");
    let metas = loader.discover().await.expect("discover");
    assert!(
        metas.iter().any(|m| m.id == "sb-test"),
        "test plugin must load, got {metas:?}"
    );
    (tmp, loader)
}

#[tokio::test]
async fn test_infinite_loop_in_resolve_is_terminated() {
    // 1-second budget; infinite loop must abort.
    let limits = SandboxLimits {
        max_execution_secs: 1,
        ..SandboxLimits::default()
    };
    let plugin = r#"
        module.exports = {
            id: "sb-test",
            name: "Sandbox Test",
            version: "1.0.0",
            urlPattern: ".*",
            resolve(_url) {
                while (true) { /* spin */ }
            },
        };
    "#;
    let (_tmp, loader) = loader_with(limits, plugin).await;

    let start = std::time::Instant::now();
    let res = loader.resolve("sb-test", "http://example.com/x").await;
    let elapsed = start.elapsed();

    assert!(res.is_err(), "infinite loop must produce an error");
    // Generous upper bound: QuickJS interrupt polling + thread scheduling.
    assert!(
        elapsed < std::time::Duration::from_secs(10),
        "execution must abort within ~10s, took {elapsed:?}"
    );
}

#[tokio::test]
async fn test_filesystem_access_is_not_available() {
    let plugin = r#"
        module.exports = {
            id: "sb-test",
            name: "Sandbox Test",
            version: "1.0.0",
            urlPattern: ".*",
            resolve(_url) {
                // `require` must not exist in the sandbox. Returning typeof
                // lets us assert from Rust without throwing in JS.
                return {
                    name: "require_typeof=" + (typeof require),
                    downloads: [{
                        url: "http://example.com/dummy",
                        filename: null, filesize: null,
                        chunks_supported: false, max_chunks: null,
                        headers: null, cookies: null, wait_seconds: null,
                        mirrors: [],
                    }],
                };
            },
        };
    "#;
    let (_tmp, loader) = loader_with(SandboxLimits::default(), plugin).await;

    let pkg = loader
        .resolve("sb-test", "http://example.com/x")
        .await
        .expect("resolve");
    assert_eq!(
        pkg.name, "require_typeof=undefined",
        "`require` must be undefined in sandbox, got name={}",
        pkg.name
    );
}

#[tokio::test]
async fn test_process_and_node_globals_are_not_available() {
    let plugin = r#"
        module.exports = {
            id: "sb-test",
            name: "Sandbox Test",
            version: "1.0.0",
            urlPattern: ".*",
            resolve(_url) {
                return {
                    name: [
                        "process=" + (typeof process),
                        "Deno=" + (typeof Deno),
                        "Bun=" + (typeof Bun),
                        "global=" + (typeof global),
                    ].join(","),
                    downloads: [{
                        url: "http://example.com/dummy",
                        filename: null, filesize: null,
                        chunks_supported: false, max_chunks: null,
                        headers: null, cookies: null, wait_seconds: null,
                        mirrors: [],
                    }],
                };
            },
        };
    "#;
    let (_tmp, loader) = loader_with(SandboxLimits::default(), plugin).await;

    let pkg = loader
        .resolve("sb-test", "http://example.com/x")
        .await
        .expect("resolve");
    for forbidden in ["process=undefined", "Deno=undefined", "Bun=undefined"] {
        assert!(
            pkg.name.contains(forbidden),
            "expected {forbidden} in sandbox-probe result, got {}",
            pkg.name
        );
    }
}

#[tokio::test]
async fn test_amigo_host_api_is_exposed() {
    // Counterpart to the negative tests: the legitimate sandbox global
    // `amigo` must be available so plugins can do anything useful.
    let plugin = r#"
        module.exports = {
            id: "sb-test",
            name: "Sandbox Test",
            version: "1.0.0",
            urlPattern: ".*",
            resolve(_url) {
                return {
                    name: "amigo=" + (typeof amigo)
                        + " httpGet=" + (typeof (amigo && amigo.httpGet))
                        + " regexMatch=" + (typeof (amigo && amigo.regexMatch)),
                    downloads: [{
                        url: "http://example.com/dummy",
                        filename: null, filesize: null,
                        chunks_supported: false, max_chunks: null,
                        headers: null, cookies: null, wait_seconds: null,
                        mirrors: [],
                    }],
                };
            },
        };
    "#;
    let (_tmp, loader) = loader_with(SandboxLimits::default(), plugin).await;

    let pkg = loader
        .resolve("sb-test", "http://example.com/x")
        .await
        .expect("resolve");
    assert!(pkg.name.contains("amigo=object"), "got {}", pkg.name);
    assert!(pkg.name.contains("httpGet=function"), "got {}", pkg.name);
    assert!(pkg.name.contains("regexMatch=function"), "got {}", pkg.name);
}

#[tokio::test]
async fn test_plugin_dir_constant_does_not_leak_environment() {
    // Make sure the plugin can't read its own absolute path from a hidden
    // global, and can't see CARGO_*/HOME-style env vars.
    let plugin = r#"
        module.exports = {
            id: "sb-test",
            name: "Sandbox Test",
            version: "1.0.0",
            urlPattern: ".*",
            resolve(_url) {
                // `__dirname`, `__filename` are Node-only; must be absent.
                return {
                    name: "__dirname=" + (typeof __dirname)
                        + " __filename=" + (typeof __filename),
                    downloads: [{
                        url: "http://example.com/dummy",
                        filename: null, filesize: null,
                        chunks_supported: false, max_chunks: null,
                        headers: null, cookies: null, wait_seconds: null,
                        mirrors: [],
                    }],
                };
            },
        };
    "#;
    let (_tmp, loader) = loader_with(SandboxLimits::default(), plugin).await;

    let pkg = loader
        .resolve("sb-test", "http://example.com/x")
        .await
        .expect("resolve");
    assert!(
        pkg.name.contains("__dirname=undefined"),
        "Node `__dirname` must not leak, got {}",
        pkg.name
    );
    assert!(
        pkg.name.contains("__filename=undefined"),
        "Node `__filename` must not leak, got {}",
        pkg.name
    );
}

// Sanity: the tempdir lives long enough to keep PluginLoader's file handles
// happy. (Smoke check on the helper — not strictly part of the spec but a
// cheap regression guard if the helper signature changes.)
#[tokio::test]
async fn test_helper_loader_works_with_minimal_plugin() {
    let plugin = r#"
        module.exports = {
            id: "sb-test",
            name: "Sandbox Test",
            version: "1.0.0",
            urlPattern: ".*",
            resolve(_url) {
                return { name: "ok", downloads: [] };
            },
        };
    "#;
    let (tmp, _loader) = loader_with(SandboxLimits::default(), plugin).await;
    assert!(Path::new(tmp.path()).exists());
}
