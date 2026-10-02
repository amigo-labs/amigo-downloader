//! Install / update flow against a mock registry: a candidate artifact is
//! validated (identity, version, permissions) before anything is written into
//! the plugin directory, so a rejected artifact can never come back on the
//! next start.

use std::path::Path;
use std::sync::Arc;

use amigo_plugin_runtime::loader::PluginLoader;
use amigo_plugin_runtime::registry::RegistryConfig;
use amigo_plugin_runtime::sandbox::SandboxLimits;
use amigo_plugin_runtime::updater::PluginUpdater;
use sha2::{Digest, Sha256};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn source(id: &str, version: &str, domains: &str) -> String {
    format!(
        r#"module.exports = {{
    id: "{id}", name: "{id}", version: "{version}", apiVersion: 1,
    permissions: {{ domains: {domains} }},
    urlPattern: "https?://{id}\\.test/.+",
    resolve(url) {{ return {{ name: "x", downloads: [] }}; }},
}};"#
    )
}

/// Serve an unsigned index with one entry for `id` whose artifact is `body`.
async fn registry(mock: &MockServer, id: &str, version: &str, declared: &str, body: &str) {
    let sha = hex::encode(Sha256::digest(body.as_bytes()));
    let index = format!(
        r#"{{"schema_version":1,"plugins":[{{"id":"{id}","name":"{id}","version":"{version}",
            "description":"","author":"","url_pattern":".*","min_app_version":null,
            "sha256":"{sha}","download_url":"{uri}/{id}.ts","tags":[],
            "api_version":1,"permissions":{{"domains":{declared}}}}}]}}"#,
        uri = mock.uri()
    );
    mock.reset().await;
    Mock::given(method("GET"))
        .and(path("/index.json"))
        .respond_with(ResponseTemplate::new(200).set_body_string(index))
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/{id}.ts")))
        .respond_with(ResponseTemplate::new(200).set_body_string(body.to_string()))
        .mount(mock)
        .await;
}

fn updater(mock: &MockServer, plugins: &Path) -> (Arc<PluginLoader>, PluginUpdater) {
    let loader =
        Arc::new(PluginLoader::new(plugins.to_path_buf(), SandboxLimits::default()).unwrap());
    let mut config = RegistryConfig::for_index(format!("{}/index.json", mock.uri()), true);
    // No index cache: each step below swaps the registry contents.
    config.cache_path = None;
    let updater = PluginUpdater::new(config, reqwest::Client::new(), Arc::clone(&loader));
    (loader, updater)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn install_rejects_artifact_claiming_another_identity() {
    let mock = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();
    let plugins = tmp.path().join("plugins");
    std::fs::create_dir_all(&plugins).unwrap();
    let (loader, updater) = updater(&mock, &plugins);

    // The registry entry is "innocent", but the artifact says it is "victim".
    registry(
        &mock,
        "innocent",
        "1.0.0",
        r#"["a.test"]"#,
        &source("victim", "1.0.0", r#"["a.test"]"#),
    )
    .await;
    let err = updater.install_plugin("innocent", true).await.unwrap_err();
    assert!(
        err.to_string().contains("declares itself as victim"),
        "{err}"
    );
    assert!(
        !plugins.join("innocent").exists(),
        "nothing written to disk"
    );
    assert!(loader.list_plugins().await.is_empty(), "nothing registered");

    // Same for a version that differs from the signed entry.
    registry(
        &mock,
        "innocent",
        "1.0.0",
        r#"["a.test"]"#,
        &source("innocent", "9.9.9", r#"["a.test"]"#),
    )
    .await;
    assert!(updater.install_plugin("innocent", true).await.is_err());
    assert!(!plugins.join("innocent").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn install_rejects_manifest_wider_than_approved_and_leaves_no_file() {
    let mock = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();
    let plugins = tmp.path().join("plugins");
    std::fs::create_dir_all(&plugins).unwrap();
    let (loader, updater) = updater(&mock, &plugins);

    // Registry (what the user approves) says a.test; the artifact wants more.
    registry(
        &mock,
        "greedy",
        "1.0.0",
        r#"["a.test"]"#,
        &source("greedy", "1.0.0", r#"["a.test", "evil.test"]"#),
    )
    .await;
    let err = updater.install_plugin("greedy", true).await.unwrap_err();
    assert!(
        matches!(
            err,
            amigo_plugin_runtime::Error::PermissionApprovalRequired(_)
        ),
        "{err}"
    );
    assert!(!plugins.join("greedy").exists());

    // A fresh loader (= daemon restart) finds nothing to load.
    let restarted = PluginLoader::new(plugins.clone(), SandboxLimits::default()).unwrap();
    assert!(restarted.discover().await.unwrap().is_empty());
    assert!(loader.list_plugins().await.is_empty());

    // Unapproved installs are refused before anything is downloaded.
    let err = updater.install_plugin("greedy", false).await.unwrap_err();
    assert!(matches!(
        err,
        amigo_plugin_runtime::Error::PermissionApprovalRequired(_)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn install_then_update_in_place_and_widening_needs_approval() {
    let mock = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();
    let plugins = tmp.path().join("plugins");
    // Installed under a category folder, like the shipped plugins.
    let installed_dir = plugins.join("hosters").join("good");
    std::fs::create_dir_all(&installed_dir).unwrap();
    std::fs::write(
        installed_dir.join("plugin.js"),
        source("good", "1.0.0", r#"["a.test"]"#),
    )
    .unwrap();
    let (loader, updater) = updater(&mock, &plugins);
    loader.discover().await.unwrap();

    // A same-scope update applies without approval, in place.
    registry(
        &mock,
        "good",
        "1.1.0",
        r#"["a.test"]"#,
        &source("good", "1.1.0", r#"["a.test"]"#),
    )
    .await;
    let meta = updater.update_plugin("good", false).await.unwrap();
    assert_eq!(meta.version, "1.1.0");
    assert!(installed_dir.join("plugin.ts").exists());
    assert!(
        !installed_dir.join("plugin.js").exists(),
        "stale entry with the other extension is removed"
    );
    assert!(
        !plugins.join("good").exists(),
        "no duplicate copy at the top level"
    );

    // A widening update is refused without approval and leaves 1.1.0 intact.
    registry(
        &mock,
        "good",
        "2.0.0",
        r#"["a.test", "b.test"]"#,
        &source("good", "2.0.0", r#"["a.test", "b.test"]"#),
    )
    .await;
    let err = updater.update_plugin("good", false).await.unwrap_err();
    assert!(matches!(
        err,
        amigo_plugin_runtime::Error::PermissionApprovalRequired(_)
    ));
    assert_eq!(
        loader.get_plugin_meta("good").await.unwrap().version,
        "1.1.0"
    );
    assert!(updater.update_all_plugins().await.unwrap().is_empty());

    // With approval it applies.
    let meta = updater.update_plugin("good", true).await.unwrap();
    assert_eq!(meta.version, "2.0.0");
    assert_eq!(
        meta.permissions.domains.as_deref(),
        Some(&["a.test".to_string(), "b.test".to_string()][..])
    );
}

#[tokio::test]
async fn throwing_api_version_getter_is_an_error_not_v1() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("p");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("plugin.js");
    std::fs::write(
        &path,
        r#"module.exports = {
    id: "p", name: "p", version: "1.0.0", urlPattern: ".*",
    get apiVersion() { throw new Error("nope"); },
    resolve(url) { return { name: "x", downloads: [] }; },
};"#,
    )
    .unwrap();
    let loader = PluginLoader::new(tmp.path().to_path_buf(), SandboxLimits::default()).unwrap();
    let err = loader.load_plugin(&path).await.unwrap_err().to_string();
    assert!(err.contains("apiVersion"), "{err}");
}
