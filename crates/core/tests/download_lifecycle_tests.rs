//! Download lifecycle integration tests with a real HTTP mock backend.
//!
//! Covers Phase 4 of `docs/specs/integration-tests.md`: drive a fresh
//! Coordinator against a wiremock-served file and assert the on-disk
//! result, retry behavior, and cancel cleanup.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use amigo_core::config::Config;
use amigo_core::coordinator::{Coordinator, DownloadEvent};
use amigo_core::storage::Storage;
use tokio::sync::broadcast::Receiver;
use tokio::time::timeout;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Register both a HEAD and GET mock for the same path. The HTTP backend
/// always issues a HEAD probe first, so a missing HEAD mock would cause
/// the download to fail before the GET handler runs.
async fn mount_static_file(mock: &MockServer, route: &str, body: &[u8]) {
    let len = body.len();
    Mock::given(method("HEAD"))
        .and(path(route))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-length", len.to_string())
                .insert_header("accept-ranges", "none"),
        )
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.to_vec()))
        .mount(mock)
        .await;
}

/// Build a coordinator with a tempdir-rooted in-memory store. The
/// tempdir's lifetime is bound to the returned tuple; drop it and the
/// download directory disappears.
fn lifecycle_coordinator() -> (tempfile::TempDir, Arc<Coordinator>) {
    lifecycle_coordinator_with(|_| {})
}

/// Like [`lifecycle_coordinator`] but lets the caller tweak the config
/// before the coordinator is built (e.g. shrink retry budgets).
fn lifecycle_coordinator_with(
    tweak: impl FnOnce(&mut Config),
) -> (tempfile::TempDir, Arc<Coordinator>) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut storage = Storage::open_memory().expect("open in-memory storage");
    storage.download_dir = tmp.path().to_path_buf();
    storage.temp_dir = tmp.path().join(".tmp");
    std::fs::create_dir_all(&storage.temp_dir).unwrap();

    let mut config = Config {
        download_dir: tmp.path().display().to_string(),
        max_concurrent_downloads: 4,
        ..Config::default()
    };
    // Default retry budget (5 attempts × exponential up to 60s) is too slow
    // for an integration test; keep it short unless the caller raises it.
    config.retry.max_retries = 3;
    config.retry.base_delay_secs = 0.05;
    config.retry.max_delay_secs = 0.2;
    tweak(&mut config);

    let coord = Arc::new(Coordinator::new(config, storage));
    coord.spawn_queue_advance_loop();
    (tmp, coord)
}

/// Await a `Completed` or `Failed` event for the given download ID. Returns
/// `true` for completion, `false` for failure. Times out after 15s.
async fn wait_for_terminal(mut rx: Receiver<DownloadEvent>, id: &str) -> Option<bool> {
    let deadline = Duration::from_secs(15);
    timeout(deadline, async move {
        loop {
            match rx.recv().await {
                Ok(DownloadEvent::Completed { id: ev_id }) if ev_id == id => return Some(true),
                Ok(DownloadEvent::Failed { id: ev_id, .. }) if ev_id == id => return Some(false),
                Ok(_) => continue,
                Err(_) => return None,
            }
        }
    })
    .await
    .ok()
    .flatten()
}

/// Read a download's on-disk file via the coordinator's storage view.
/// If the row carries no `filename`, fall back to scanning the download
/// directory for the single regular file produced by this run.
async fn read_completed_file(coord: &Coordinator, id: &str) -> Vec<u8> {
    let row = coord
        .storage()
        .get_download(id)
        .await
        .unwrap()
        .expect("download row");
    let dir: PathBuf = row
        .download_dir
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| coord.storage().download_dir.clone());

    let path = match row.filename.as_deref() {
        Some(name) => dir.join(name),
        None => {
            // Pick the only non-hidden file in the dir.
            let mut files = std::fs::read_dir(&dir)
                .unwrap_or_else(|e| panic!("read_dir {} failed: {e}", dir.display()))
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file()
                        && p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| !n.starts_with('.'))
                })
                .collect::<Vec<_>>();
            assert_eq!(
                files.len(),
                1,
                "expected exactly one downloaded file in {}, found {:?}",
                dir.display(),
                files
            );
            files.pop().unwrap()
        }
    };
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {} failed: {e}", path.display()))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_simple_download_writes_file_to_disk() {
    let mock = MockServer::start().await;
    mount_static_file(&mock, "/file.bin", b"hello world").await;

    let (_tmp, coord) = lifecycle_coordinator();
    let rx = coord.subscribe();
    let id = coord
        .add_download(&format!("{}/file.bin", mock.uri()), None)
        .await
        .expect("add_download");

    let ok = wait_for_terminal(rx, &id).await;
    assert_eq!(ok, Some(true), "download must complete");

    let bytes = read_completed_file(&coord, &id).await;
    assert_eq!(bytes, b"hello world");
}

// Known-broken: tracked by audit #13 follow-up in
// `docs/specs/audit-2026-04-25.md` — the coordinator's per-attempt cancel
// oneshot is fabricated fresh after attempt 0 and the dummy sender is
// dropped immediately, which closes the receiver and produces a
// "Download cancelled" error on every retry. Fixing that bug will make
// this test pass; run with `cargo test -- --ignored` to verify.
#[ignore = "audit #13: retry loop self-cancels because cancel oneshot is recreated and dropped"]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_download_retries_on_transient_5xx() {
    let mock = MockServer::start().await;
    let body = b"recovered";
    // First HEAD probe fails with 503; subsequent ones succeed. wiremock
    // resolves overlapping mocks by exhausting `up_to_n_times` first, so
    // the unlimited 200-mock only kicks in once the 503-mock is spent.
    Mock::given(method("HEAD"))
        .and(path("/flaky.bin"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .mount(&mock)
        .await;
    Mock::given(method("HEAD"))
        .and(path("/flaky.bin"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-length", body.len().to_string())
                .insert_header("accept-ranges", "none"),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/flaky.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.to_vec()))
        .mount(&mock)
        .await;

    let (_tmp, coord) = lifecycle_coordinator();
    let rx = coord.subscribe();
    let id = coord
        .add_download(&format!("{}/flaky.bin", mock.uri()), None)
        .await
        .expect("add_download");

    let outcome = wait_for_terminal(rx, &id).await;
    if outcome != Some(true) {
        // Surface the stored error so the failure mode is obvious.
        let row = coord.storage().get_download(&id).await.unwrap().unwrap();
        panic!(
            "download did not complete after retries: outcome={outcome:?} status={} error={:?}",
            row.status, row.error_message
        );
    }

    let bytes = read_completed_file(&coord, &id).await;
    assert_eq!(bytes, b"recovered");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_permanent_failure_marks_download_failed() {
    let mock = MockServer::start().await;
    Mock::given(method("HEAD"))
        .and(path("/gone.bin"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/gone.bin"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;

    // Tight retry budget so the test finishes quickly even though every
    // attempt hits 404.
    let (_tmp, coord) = lifecycle_coordinator_with(|c| {
        c.retry.max_retries = 1;
        c.retry.base_delay_secs = 0.01;
        c.retry.max_delay_secs = 0.05;
    });
    let rx = coord.subscribe();
    let id = coord
        .add_download(&format!("{}/gone.bin", mock.uri()), None)
        .await
        .expect("add_download");

    let outcome = wait_for_terminal(rx, &id).await;
    assert_eq!(
        outcome,
        Some(false),
        "404 must propagate to Failed (not Completed)"
    );

    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.status, "failed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_cancel_removes_download_row() {
    // HEAD must succeed so the backend actually starts a transfer; the GET
    // is deliberately stalled so we can cancel mid-flight before any bytes
    // arrive.
    let body = b"never-seen";
    let mock = MockServer::start().await;
    Mock::given(method("HEAD"))
        .and(path("/slow.bin"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-length", body.len().to_string())
                .insert_header("accept-ranges", "none"),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/slow.bin"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(10))
                .set_body_bytes(body.to_vec()),
        )
        .mount(&mock)
        .await;

    let (_tmp, coord) = lifecycle_coordinator();
    let id = coord
        .add_download(&format!("{}/slow.bin", mock.uri()), None)
        .await
        .expect("add_download");

    coord.cancel(&id).await.expect("cancel");

    // After cancel, the row should no longer be queryable.
    let row = coord.storage().get_download(&id).await.unwrap();
    assert!(
        row.is_none(),
        "cancelled download must be removed from storage, got {row:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_pause_does_not_mark_failed() {
    // Regression: pausing an in-flight download used to end up "failed",
    // because the download task's error arm overwrote the Paused status when
    // the transfer returned Cancelled.
    let body = b"never-seen";
    let mock = MockServer::start().await;
    Mock::given(method("HEAD"))
        .and(path("/slow.bin"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-length", body.len().to_string())
                .insert_header("accept-ranges", "none"),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/slow.bin"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(10))
                .set_body_bytes(body.to_vec()),
        )
        .mount(&mock)
        .await;

    let (_tmp, coord) = lifecycle_coordinator();
    let id = coord
        .add_download(&format!("{}/slow.bin", mock.uri()), None)
        .await
        .expect("add_download");

    // Let the backend start the (stalled) transfer, then pause it.
    tokio::time::sleep(Duration::from_millis(300)).await;
    coord.pause(&id).await.expect("pause");

    // Give the download task time to observe the cancellation and finish.
    tokio::time::sleep(Duration::from_millis(600)).await;

    let row = coord
        .storage()
        .get_download(&id)
        .await
        .unwrap()
        .expect("paused download row must still exist");
    assert_eq!(
        row.status, "paused",
        "paused download must stay 'paused', not become 'failed'"
    );
}
