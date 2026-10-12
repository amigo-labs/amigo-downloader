//! Tests for the out-of-process plugin host (issue #79, SH-7a): plugin JS
//! runs in `amigo-plugin-host`, a dying child fails only the calls it was
//! running, and the daemon keeps every piece of state that must survive it.
#![cfg(unix)]

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use amigo_plugin_runtime::Error;
use amigo_plugin_runtime::host_api::{CaptchaFuture, HostApi};
use amigo_plugin_runtime::loader::PluginLoader;
use amigo_plugin_runtime::sandbox::SandboxLimits;

fn write_plugin(root: &Path, id: &str, resolve_body: &str) {
    let dir = root.join(id);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("plugin.js"),
        format!(
            r#"module.exports = {{
    id: "{id}",
    name: "{id}",
    version: "1.0.0",
    apiVersion: 1,
    permissions: {{ domains: ["{id}.test"] }},
    urlPattern: "https?://{id}\\.test/.+",
    resolve(url) {{
        {resolve_body}
    }},
}};
"#
        ),
    )
    .unwrap();
}

/// `resolve()` body returning one download whose filename is `expr`.
fn returning(expr: &str) -> String {
    format!(
        r#"return {{ name: "x", downloads: [{{ url: url, filename: {expr}, filesize: null, chunks_supported: true, max_chunks: null, headers: null, cookies: null, wait_seconds: null, mirrors: [] }}] }};"#
    )
}

fn limits(exec_secs: u64) -> SandboxLimits {
    SandboxLimits {
        max_execution_secs: exec_secs,
        ..SandboxLimits::default()
    }
}

fn signal(pid: u32, sig: libc::c_int) {
    let pid = libc::pid_t::try_from(pid).unwrap();
    // SAFETY: plain kill(2) on a child process this test spawned.
    assert_eq!(unsafe { libc::kill(pid, sig) }, 0, "kill({pid}, {sig})");
}

async fn filename(loader: &PluginLoader, id: &str, url: &str) -> Result<Option<String>, Error> {
    let pkg = loader.resolve(id, url).await?;
    Ok(pkg.downloads[0].filename.clone())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn daemon_runs_plugins_in_a_child_process() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(tmp.path(), "plain", &returning("\"ok.bin\""));
    let loader = PluginLoader::new(tmp.path().to_path_buf(), limits(5)).unwrap();

    assert_eq!(loader.plugin_host_pid().await, None, "spawned lazily");
    assert_eq!(loader.discover().await.unwrap().len(), 1);

    let pid = loader.plugin_host_pid().await.expect("plugin host running");
    assert_ne!(pid, std::process::id());
    assert_eq!(
        filename(&loader, "plain", "https://plain.test/f")
            .await
            .unwrap(),
        Some("ok.bin".into())
    );
    assert_eq!(
        loader.plugin_host_pid().await,
        Some(pid),
        "one shared child"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn killed_child_fails_the_call_and_marks_the_plugin_failing() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(
        tmp.path(),
        "flaky",
        &format!(
            "if (url.indexOf('spin') >= 0) {{ while (true) {{}} }}\n{}",
            returning("\"flaky.bin\"")
        ),
    );
    write_plugin(tmp.path(), "steady", &returning("\"steady.bin\""));
    let loader = Arc::new(PluginLoader::new(tmp.path().to_path_buf(), limits(20)).unwrap());
    loader.discover().await.unwrap();
    let first = loader.plugin_host_pid().await.unwrap();

    let spinning = {
        let loader = Arc::clone(&loader);
        tokio::spawn(async move { loader.resolve("flaky", "https://flaky.test/spin").await })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    signal(first, libc::SIGKILL);

    let err = tokio::time::timeout(Duration::from_secs(10), spinning)
        .await
        .expect("the in-flight call fails promptly")
        .unwrap()
        .unwrap_err();
    assert!(matches!(err, Error::HostCrashed(_)), "got {err:?}");
    assert!(err.to_string().contains("flaky"), "names the plugin: {err}");
    assert!(loader.get_plugin_meta("flaky").await.unwrap().failing);
    assert!(!loader.get_plugin_meta("steady").await.unwrap().failing);

    // The daemon and its plugin list survived; the next call gets a new child.
    assert_eq!(loader.list_plugins().await.len(), 2);
    assert_eq!(
        filename(&loader, "steady", "https://steady.test/f")
            .await
            .unwrap(),
        Some("steady.bin".into())
    );
    let second = loader.plugin_host_pid().await.unwrap();
    assert_ne!(first, second);

    // A successful call clears the mark.
    assert_eq!(
        filename(&loader, "flaky", "https://flaky.test/f")
            .await
            .unwrap(),
        Some("flaky.bin".into())
    );
    assert!(!loader.get_plugin_meta("flaky").await.unwrap().failing);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plugin_state_survives_a_child_restart() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(
        tmp.path(),
        "stateful",
        &format!(
            r#"if (url.indexOf('write') >= 0) {{
            amigo.storageSet("token", "t-123");
            amigo.setCookie("stateful.test", "sid", "c-456");
        }}
        {}"#,
            returning(
                r#"amigo.storageGet("token") + "/" + amigo.getCookie("stateful.test", "sid")"#
            )
        ),
    );
    let loader = PluginLoader::new(tmp.path().to_path_buf(), limits(5)).unwrap();
    loader.discover().await.unwrap();

    assert_eq!(
        filename(&loader, "stateful", "https://stateful.test/write")
            .await
            .unwrap(),
        Some("t-123/c-456".into())
    );

    let pid = loader.plugin_host_pid().await.unwrap();
    signal(pid, libc::SIGKILL);
    // Let the reader observe the exit, so the next call starts a new child.
    for _ in 0..50 {
        if loader.plugin_host_pid().await.is_none() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    assert_eq!(
        filename(&loader, "stateful", "https://stateful.test/read")
            .await
            .unwrap(),
        Some("t-123/c-456".into()),
        "storage and cookies live in the daemon"
    );
    assert_ne!(loader.plugin_host_pid().await, Some(pid));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stopped_child_is_killed_by_the_watchdog() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(tmp.path(), "spin", "while (true) {}");
    let loader = Arc::new(PluginLoader::new(tmp.path().to_path_buf(), limits(1)).unwrap());
    loader.set_watchdog_grace(Duration::from_millis(500));
    loader.discover().await.unwrap();
    let pid = loader.plugin_host_pid().await.unwrap();

    let call = {
        let loader = Arc::clone(&loader);
        tokio::spawn(async move { loader.resolve("spin", "https://spin.test/f").await })
    };
    tokio::time::sleep(Duration::from_millis(100)).await;
    // A stopped child cannot run its own interrupt handler; only the
    // daemon-side watchdog can end the call.
    signal(pid, libc::SIGSTOP);

    let err = tokio::time::timeout(Duration::from_secs(10), call)
        .await
        .expect("watchdog fires")
        .unwrap()
        .unwrap_err();
    assert!(matches!(err, Error::HostCrashed(_)), "got {err:?}");
    assert!(err.to_string().contains("unresponsive"), "{err}");
}

fn captcha_host(delay: Duration) -> HostApi {
    let mut host = HostApi::from_sandbox(&SandboxLimits::default());
    host.set_captcha_callback(Arc::new(
        move |plugin_id, _download_id, image_url, captcha_type| -> CaptchaFuture {
            Box::pin(async move {
                tokio::time::sleep(delay).await;
                Ok(format!("{plugin_id}:{image_url}:{captcha_type}"))
            })
        },
    ));
    host
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn captcha_callback_round_trips_through_the_parent() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(
        tmp.path(),
        "cap",
        &returning(r#"amigo.solveCaptcha("https://cap.test/img.png")"#),
    );
    let loader = PluginLoader::new_with_host_api(
        tmp.path().to_path_buf(),
        limits(5),
        captcha_host(Duration::ZERO),
    )
    .unwrap();
    loader.discover().await.unwrap();

    assert_eq!(
        filename(&loader, "cap", "https://cap.test/f")
            .await
            .unwrap(),
        Some("cap:https://cap.test/img.png:image".into())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_pending_captcha_does_not_trip_the_watchdog() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(
        tmp.path(),
        "slowcap",
        r#"amigo.solveCaptcha("https://slowcap.test/img.png"); while (true) {}"#,
    );
    let loader = PluginLoader::new_with_host_api(
        tmp.path().to_path_buf(),
        limits(1),
        captcha_host(Duration::from_secs(3)),
    )
    .unwrap();
    loader.set_watchdog_grace(Duration::from_millis(500));
    loader.discover().await.unwrap();
    let pid = loader.plugin_host_pid().await.unwrap();

    // The human takes longer than budget + grace. The watchdog must not
    // count that; the plugin's own deadline ends the call once JS resumes.
    let err = loader
        .resolve("slowcap", "https://slowcap.test/f")
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Timeout(_)), "got {err:?}");
    assert_eq!(
        loader.plugin_host_pid().await,
        Some(pid),
        "child not killed"
    );
}

#[tokio::test]
async fn missing_host_binary_is_a_clear_error() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(tmp.path(), "orphan", &returning("null"));
    let loader = PluginLoader::new(tmp.path().to_path_buf(), limits(5))
        .unwrap()
        .with_plugin_host_binary("/nonexistent/amigo-plugin-host".into());

    let err = loader
        .load_plugin(&tmp.path().join("orphan/plugin.js"))
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("failed to start plugin host"),
        "{err}"
    );
    // discover() logs the failure and keeps the daemon running.
    assert!(loader.discover().await.unwrap().is_empty());
}
