//! Host-API integration tests.
//!
//! Covers Phase 2.3 of `docs/specs/integration-tests.md`: drive the
//! `amigo.*` host functions from inside the QuickJS sandbox and assert
//! the side effects observed on a wiremock server, plus the
//! private-network guard and the storage quota.

use amigo_plugin_runtime::loader::PluginLoader;
use amigo_plugin_runtime::sandbox::SandboxLimits;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Build a loader rooted at a fresh tempdir containing a single plugin.
/// The plugin's resolve() returns its result encoded into `name`, so tests
/// can assert against the package returned by `loader.resolve(...)`.
async fn loader_with(
    limits: SandboxLimits,
    plugin_source: &str,
) -> (tempfile::TempDir, PluginLoader) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("ha-test");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("plugin.js"), plugin_source).unwrap();
    let loader = PluginLoader::new(tmp.path().to_path_buf(), limits).expect("loader init");
    let metas = loader.discover().await.expect("discover");
    assert!(
        metas.iter().any(|m| m.id == "ha-test"),
        "test plugin must load, got {metas:?}"
    );
    (tmp, loader)
}

/// Sandbox limits that allow private network (so the plugin can reach
/// the wiremock server on 127.0.0.1).
fn permissive_limits() -> SandboxLimits {
    SandboxLimits {
        allow_private_network: true,
        ..SandboxLimits::default()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_http_get_through_host_api_reaches_mock() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/file"))
        .respond_with(ResponseTemplate::new(200).set_body_string("hello-world"))
        .mount(&mock)
        .await;

    let plugin = format!(
        r#"
        module.exports = {{
            id: "ha-test",
            name: "Host API Test",
            version: "1.0.0",
            urlPattern: ".*",
            resolve(_url) {{
                var resp = amigo.httpGet("{base}/file");
                return {{
                    name: "status=" + resp.status + " body=" + resp.body,
                    downloads: [{{
                        url: "{base}/file",
                        filename: null, filesize: null,
                        chunks_supported: false, max_chunks: null,
                        headers: null, cookies: null, wait_seconds: null,
                        mirrors: [],
                    }}],
                }};
            }},
        }};
        "#,
        base = mock.uri()
    );

    let (_tmp, loader) = loader_with(permissive_limits(), &plugin).await;
    let pkg = loader
        .resolve("ha-test", "http://example.com/")
        .await
        .expect("resolve");
    assert!(pkg.name.contains("status=200"), "got {}", pkg.name);
    assert!(pkg.name.contains("body=hello-world"), "got {}", pkg.name);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_http_post_form_is_received_by_mock() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/submit"))
        .respond_with(ResponseTemplate::new(201).set_body_string("created"))
        .mount(&mock)
        .await;

    let plugin = format!(
        r#"
        module.exports = {{
            id: "ha-test",
            name: "Host API Test",
            version: "1.0.0",
            urlPattern: ".*",
            resolve(_url) {{
                var resp = amigo.httpPostForm(
                    "{base}/submit",
                    {{ a: "1", b: "two" }}
                );
                return {{
                    name: "status=" + resp.status,
                    downloads: [{{
                        url: "{base}/submit",
                        filename: null, filesize: null,
                        chunks_supported: false, max_chunks: null,
                        headers: null, cookies: null, wait_seconds: null,
                        mirrors: [],
                    }}],
                }};
            }},
        }};
        "#,
        base = mock.uri()
    );

    let (_tmp, loader) = loader_with(permissive_limits(), &plugin).await;
    let pkg = loader
        .resolve("ha-test", "http://example.com/")
        .await
        .expect("resolve");
    assert!(pkg.name.contains("status=201"), "got {}", pkg.name);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_private_network_blocked_by_default() {
    // SSRF guard: with default limits (allow_private_network=false), a
    // plugin reaching 127.0.0.1 should be denied — even though wiremock
    // is running there. The exact failure mode is implementation-defined;
    // we only require that resolve() does not return a 200 from the mock.
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/blocked"))
        .respond_with(ResponseTemplate::new(200).set_body_string("should-not-see"))
        .mount(&mock)
        .await;

    let plugin = format!(
        r#"
        module.exports = {{
            id: "ha-test",
            name: "Host API Test",
            version: "1.0.0",
            urlPattern: ".*",
            resolve(_url) {{
                try {{
                    var resp = amigo.httpGet("{base}/blocked");
                    return {{
                        name: "got=" + resp.status + ":" + resp.body,
                        downloads: [],
                    }};
                }} catch (e) {{
                    return {{
                        name: "blocked:" + (e && e.message ? e.message : String(e)),
                        downloads: [],
                    }};
                }}
            }},
        }};
        "#,
        base = mock.uri()
    );

    // Default sandbox limits — allow_private_network = false.
    let (_tmp, loader) = loader_with(SandboxLimits::default(), &plugin).await;
    let pkg = loader
        .resolve("ha-test", "http://example.com/")
        .await
        .expect("resolve");
    assert!(
        !pkg.name.contains("should-not-see"),
        "private-network request must NOT succeed, got {}",
        pkg.name
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_storage_get_set_roundtrip() {
    let plugin = r#"
        module.exports = {
            id: "ha-test",
            name: "Host API Test",
            version: "1.0.0",
            urlPattern: ".*",
            resolve(_url) {
                amigo.storageSet("hello", "world");
                amigo.storageSet("count", "42");
                var v1 = amigo.storageGet("hello");
                var v2 = amigo.storageGet("count");
                var missing = amigo.storageGet("nope");
                amigo.storageDelete("hello");
                var afterDelete = amigo.storageGet("hello");
                return {
                    name: "v1=" + v1 + " v2=" + v2
                        + " missing=" + (missing === null ? "null" : missing)
                        + " afterDel=" + (afterDelete === null ? "null" : afterDelete),
                    downloads: [],
                };
            },
        };
    "#;
    let (_tmp, loader) = loader_with(SandboxLimits::default(), plugin).await;
    let pkg = loader
        .resolve("ha-test", "http://example.com/")
        .await
        .expect("resolve");
    assert!(pkg.name.contains("v1=world"), "got {}", pkg.name);
    assert!(pkg.name.contains("v2=42"), "got {}", pkg.name);
    // Missing-key reads may come back as JS `null` or `undefined` depending
    // on the QuickJS binding — both are valid "absent" sentinels.
    let absent_ok = pkg.name.contains("missing=null") || pkg.name.contains("missing=undefined");
    assert!(
        absent_ok,
        "expected missing=null|undefined, got {}",
        pkg.name
    );
    let after_ok = pkg.name.contains("afterDel=null") || pkg.name.contains("afterDel=undefined");
    assert!(
        after_ok,
        "expected afterDel=null|undefined, got {}",
        pkg.name
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_storage_quota_enforced() {
    // Tiny quota: writing more than ~256 bytes should fail eventually.
    let limits = SandboxLimits {
        max_storage_bytes: 256,
        ..SandboxLimits::default()
    };
    let plugin = r#"
        module.exports = {
            id: "ha-test",
            name: "Host API Test",
            version: "1.0.0",
            urlPattern: ".*",
            resolve(_url) {
                var blob = "";
                for (var i = 0; i < 100; i++) blob += "abcdefghij"; // 1000 chars
                var threw = false;
                var msg = "";
                try {
                    amigo.storageSet("big", blob);
                } catch (e) {
                    threw = true;
                    msg = (e && e.message) ? e.message : String(e);
                }
                return {
                    name: "threw=" + threw + " msg=" + msg,
                    downloads: [],
                };
            },
        };
    "#;
    let (_tmp, loader) = loader_with(limits, plugin).await;
    let pkg = loader
        .resolve("ha-test", "http://example.com/")
        .await
        .expect("resolve");
    assert!(
        pkg.name.contains("threw=true"),
        "1000-byte write into 256-byte quota must throw, got {}",
        pkg.name
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_regex_and_html_helpers_work() {
    let plugin = r#"
        module.exports = {
            id: "ha-test",
            name: "Host API Test",
            version: "1.0.0",
            urlPattern: ".*",
            resolve(_url) {
                var m = amigo.regexMatch("foo-(\\d+)", "x foo-123 y");
                var attr = amigo.htmlQueryAttr(
                    "<a class=\"dl\" href=\"/file.zip\">go</a>",
                    "a.dl", "href"
                );
                var text = amigo.htmlQueryText(
                    "<h1>Hello</h1>", "h1"
                );
                var b64 = amigo.base64Encode("hi");
                var dec = amigo.base64Decode(b64);
                return {
                    name: "m=" + m + " attr=" + attr + " text=" + text
                        + " b64=" + b64 + " dec=" + dec,
                    downloads: [],
                };
            },
        };
    "#;
    let (_tmp, loader) = loader_with(SandboxLimits::default(), plugin).await;
    let pkg = loader
        .resolve("ha-test", "http://example.com/")
        .await
        .expect("resolve");
    assert!(pkg.name.contains("m=123"), "got {}", pkg.name);
    assert!(pkg.name.contains("attr=/file.zip"), "got {}", pkg.name);
    assert!(pkg.name.contains("text=Hello"), "got {}", pkg.name);
    assert!(pkg.name.contains("b64=aGk="), "got {}", pkg.name);
    assert!(pkg.name.contains("dec=hi"), "got {}", pkg.name);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_http_get_follows_redirect_chain_manually() {
    // The plugin HTTP client is built with redirect(Policy::none()); redirects
    // are followed manually in `execute_following_redirects`. This locks in
    // that a normal 3xx chain still resolves to the final body.
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/start"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("Location", format!("{}/middle", mock.uri())),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/middle"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("Location", format!("{}/end", mock.uri())),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/end"))
        .respond_with(ResponseTemplate::new(200).set_body_string("final-body"))
        .mount(&mock)
        .await;

    let plugin = format!(
        r#"
        module.exports = {{
            id: "ha-test", name: "x", version: "1.0.0", urlPattern: ".*",
            resolve(_url) {{
                var resp = amigo.httpGet("{base}/start");
                return {{ name: "status=" + resp.status + " body=" + resp.body, downloads: [] }};
            }},
        }};
        "#,
        base = mock.uri()
    );
    let (_tmp, loader) = loader_with(permissive_limits(), &plugin).await;
    let pkg = loader
        .resolve("ha-test", "http://example.com/")
        .await
        .expect("resolve");
    assert!(pkg.name.contains("status=200"), "got {}", pkg.name);
    assert!(pkg.name.contains("body=final-body"), "got {}", pkg.name);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_redirect_target_is_revalidated_against_ssrf_policy() {
    // A public host that 302-redirects to an internal/disallowed target is the
    // classic SSRF-via-redirect bypass. Even under permissive limits (the
    // initial loopback hop is allowed), the redirect target is re-validated per
    // hop — here a non-http(s) scheme, which is rejected regardless of the
    // private-network setting.
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/start"))
        .respond_with(ResponseTemplate::new(302).insert_header("Location", "file:///etc/passwd"))
        .mount(&mock)
        .await;

    let plugin = format!(
        r#"
        module.exports = {{
            id: "ha-test", name: "x", version: "1.0.0", urlPattern: ".*",
            resolve(_url) {{
                try {{
                    var resp = amigo.httpGet("{base}/start");
                    return {{ name: "got=" + resp.status, downloads: [] }};
                }} catch (e) {{
                    return {{ name: "blocked:" + (e && e.message ? e.message : String(e)), downloads: [] }};
                }}
            }},
        }};
        "#,
        base = mock.uri()
    );
    let (_tmp, loader) = loader_with(permissive_limits(), &plugin).await;
    let pkg = loader
        .resolve("ha-test", "http://example.com/")
        .await
        .expect("resolve");
    assert!(
        pkg.name.starts_with("blocked:"),
        "redirect target must be re-validated and rejected, got {}",
        pkg.name
    );
}
