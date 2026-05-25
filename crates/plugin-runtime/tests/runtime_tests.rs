//! Plugin runtime integration tests — discovery, loading, transpilation.
//!
//! Covers Phase 2.1 of `docs/specs/integration-tests.md`: the loader's
//! lifecycle against both real shipped plugins and synthetic bad inputs.

use std::path::PathBuf;

use amigo_plugin_runtime::loader::PluginLoader;
use amigo_plugin_runtime::sandbox::SandboxLimits;
use amigo_plugin_runtime::transpiler::{is_typescript, transpile};

/// Absolute path to the repository's top-level `plugins/` directory.
fn shipped_plugins_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/plugin-runtime should be two levels below repo root")
        .join("plugins")
}

#[tokio::test]
async fn test_discovers_shipped_plugins() {
    let loader =
        PluginLoader::new(shipped_plugins_dir(), SandboxLimits::default()).expect("loader init");
    let metas = loader.discover().await.expect("discover");

    let ids: Vec<&str> = metas.iter().map(|m| m.id.as_str()).collect();
    assert!(
        ids.contains(&"generic-http"),
        "expected generic-http in {ids:?}"
    );
    assert!(ids.contains(&"youtube"), "expected youtube in {ids:?}");
}

#[tokio::test]
async fn test_generic_http_plugin_metadata() {
    let loader =
        PluginLoader::new(shipped_plugins_dir(), SandboxLimits::default()).expect("loader init");
    loader.discover().await.expect("discover");

    let meta = loader
        .get_plugin_meta("generic-http")
        .await
        .expect("generic-http loaded");
    assert_eq!(meta.id, "generic-http");
    assert!(!meta.name.is_empty());
    assert!(!meta.version.is_empty());
    assert!(!meta.url_pattern.is_empty());
}

#[tokio::test]
async fn test_youtube_plugin_metadata() {
    let loader =
        PluginLoader::new(shipped_plugins_dir(), SandboxLimits::default()).expect("loader init");
    loader.discover().await.expect("discover");

    let meta = loader
        .get_plugin_meta("youtube")
        .await
        .expect("youtube loaded");
    assert_eq!(meta.id, "youtube");
    assert!(
        meta.url_pattern.contains("youtu"),
        "youtube url_pattern should mention youtu, got {}",
        meta.url_pattern
    );
}

#[tokio::test]
async fn test_match_url_finds_youtube() {
    let loader =
        PluginLoader::new(shipped_plugins_dir(), SandboxLimits::default()).expect("loader init");
    loader.discover().await.expect("discover");

    let matched = loader
        .match_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ")
        .await
        .expect("youtube should match");
    // It may match youtube directly or fall back to generic-http; both
    // are valid hosters. The assertion is that *some* plugin claims the
    // URL — match_url returning None would mean the registry is broken.
    assert!(["youtube", "generic-http"].contains(&matched.id.as_str()));
}

#[tokio::test]
async fn test_transpile_strips_typescript_syntax() {
    let ts = r#"
        interface Foo { bar: string }
        enum Color { Red, Green, Blue }
        const x: number = 42;
        function greet(name: string): string { return "hi " + name; }
    "#;
    let js = transpile(ts, "test.ts").expect("transpile");
    assert!(
        !js.contains("interface "),
        "interface keyword should be stripped: {js}"
    );
    assert!(
        !js.contains(": number"),
        "type annotations should be stripped: {js}"
    );
    assert!(js.contains("function greet"), "function survives: {js}");
}

#[tokio::test]
async fn test_transpile_rejects_invalid_syntax() {
    let bad = "this is { not valid (((";
    let res = transpile(bad, "broken.ts");
    assert!(res.is_err(), "transpile of garbage must error, got {res:?}");
}

#[test]
fn test_is_typescript_detects_extensions() {
    assert!(is_typescript(std::path::Path::new("foo.ts")));
    assert!(is_typescript(std::path::Path::new("dir/sub/foo.ts")));
    assert!(!is_typescript(std::path::Path::new("foo.js")));
    assert!(!is_typescript(std::path::Path::new("foo")));
}

#[tokio::test]
async fn test_discover_skips_invalid_plugin_without_panic() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let bad = tmp.path().join("broken-plugin");
    std::fs::create_dir(&bad).unwrap();
    std::fs::write(
        bad.join("plugin.js"),
        "this is definitely not valid JavaScript {{{ ;;",
    )
    .unwrap();

    let loader =
        PluginLoader::new(tmp.path().to_path_buf(), SandboxLimits::default()).expect("loader init");
    let metas = loader.discover().await.expect("discover must not panic");
    assert!(
        metas.is_empty(),
        "broken plugin must not be loaded, got {metas:?}"
    );
}

#[tokio::test]
async fn test_discover_skips_plugin_missing_required_exports() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("no-resolve");
    std::fs::create_dir(&dir).unwrap();
    // Valid JS but missing the required `resolve` function.
    std::fs::write(
        dir.join("plugin.js"),
        r#"module.exports = {
            id: "no-resolve",
            name: "No Resolve",
            version: "1.0.0",
            urlPattern: "https?://example\\.com/.*",
        };"#,
    )
    .unwrap();

    let loader =
        PluginLoader::new(tmp.path().to_path_buf(), SandboxLimits::default()).expect("loader init");
    let metas = loader.discover().await.expect("discover");
    assert!(
        metas.iter().all(|m| m.id != "no-resolve"),
        "plugin without resolve() must not load, got {metas:?}"
    );
}

#[tokio::test]
async fn test_discover_loads_typescript_plugin_from_tempdir() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("hello-ts");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(
        dir.join("plugin.ts"),
        r#"
            interface Out { foo: string }
            module.exports = {
                id: "hello-ts",
                name: "Hello TS",
                version: "1.0.0",
                urlPattern: "https?://example\\.com/.*",
                resolve(_url: string): { name: string; downloads: any[] } {
                    return { name: "test", downloads: [] };
                },
            };
        "#,
    )
    .unwrap();

    let loader =
        PluginLoader::new(tmp.path().to_path_buf(), SandboxLimits::default()).expect("loader init");
    let metas = loader.discover().await.expect("discover");
    assert!(
        metas.iter().any(|m| m.id == "hello-ts"),
        "TS plugin should be loaded, got {metas:?}"
    );
}

#[tokio::test]
async fn test_discover_empty_directory_returns_no_plugins() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let loader =
        PluginLoader::new(tmp.path().to_path_buf(), SandboxLimits::default()).expect("loader init");
    let metas = loader.discover().await.expect("discover");
    assert!(metas.is_empty(), "empty dir → no plugins, got {metas:?}");
}

#[tokio::test]
async fn test_discover_skips_special_dirs() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // The loader explicitly skips "types" and "template" dirs.
    let types = tmp.path().join("types");
    std::fs::create_dir(&types).unwrap();
    std::fs::write(types.join("plugin.ts"), "// pretend type file").unwrap();
    let template = tmp.path().join("template");
    std::fs::create_dir(&template).unwrap();
    std::fs::write(template.join("plugin.ts"), "// pretend template").unwrap();

    let loader =
        PluginLoader::new(tmp.path().to_path_buf(), SandboxLimits::default()).expect("loader init");
    let metas = loader.discover().await.expect("discover");
    assert!(
        metas.is_empty(),
        "types/ and template/ must be skipped, got {metas:?}"
    );
}
