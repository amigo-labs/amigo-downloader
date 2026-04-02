//! Integration tests for the storage layer.
//!
//! Tests cover edge cases, persistence, history, and CRUD operations
//! that go beyond the basic unit tests in storage.rs.

use std::path::PathBuf;

use amigo_core::chunk::{ChunkPlan, ChunkStatus};
use amigo_core::queue::QueueStatus;
use amigo_core::storage::{DownloadRow, RssFeedRow, Storage, UsenetServerRow};

/// Helper to create a DownloadRow with sensible defaults.
fn make_download(id: &str, url: &str) -> DownloadRow {
    DownloadRow {
        id: id.to_string(),
        url: url.to_string(),
        protocol: "http".to_string(),
        filename: Some(format!("{id}.zip")),
        filesize: Some(1024),
        status: "queued".to_string(),
        priority: 0,
        package_id: None,
        plugin_id: None,
        download_dir: Some("downloads".to_string()),
        bytes_downloaded: 0,
        speed_current: 0,
        error_message: None,
        retry_count: 0,
        created_at: String::new(),
        started_at: None,
        completed_at: None,
    }
}

// =============================================================================
// Basic CRUD
// =============================================================================

#[tokio::test]
async fn test_insert_and_get_download() {
    let storage = Storage::open_memory().unwrap();
    let row = make_download("dl-1", "https://example.com/file.zip");
    storage.insert_download(&row).await.unwrap();

    let fetched = storage.get_download("dl-1").await.unwrap().unwrap();
    assert_eq!(fetched.url, "https://example.com/file.zip");
    assert_eq!(fetched.filename.as_deref(), Some("dl-1.zip"));
    assert_eq!(fetched.status, "queued");
}

#[tokio::test]
async fn test_get_nonexistent_download_returns_none() {
    let storage = Storage::open_memory().unwrap();
    let result = storage.get_download("nonexistent").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_delete_download() {
    let storage = Storage::open_memory().unwrap();
    let row = make_download("dl-del", "https://example.com/file.zip");
    storage.insert_download(&row).await.unwrap();
    storage.delete_download("dl-del").await.unwrap();

    let result = storage.get_download("dl-del").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_list_downloads_order() {
    let storage = Storage::open_memory().unwrap();

    let mut dl1 = make_download("dl-a", "https://example.com/a.zip");
    dl1.priority = 1;
    let mut dl2 = make_download("dl-b", "https://example.com/b.zip");
    dl2.priority = 5;

    storage.insert_download(&dl1).await.unwrap();
    storage.insert_download(&dl2).await.unwrap();

    let list = storage.list_downloads().await.unwrap();
    assert_eq!(list.len(), 2);
    // Higher priority should come first
    assert_eq!(list[0].id, "dl-b");
    assert_eq!(list[1].id, "dl-a");
}

// =============================================================================
// Status updates
// =============================================================================

#[tokio::test]
async fn test_update_status_to_downloading_sets_started_at() {
    let storage = Storage::open_memory().unwrap();
    let row = make_download("dl-s", "https://example.com/file.zip");
    storage.insert_download(&row).await.unwrap();

    storage
        .update_download_status("dl-s", QueueStatus::Downloading)
        .await
        .unwrap();

    let fetched = storage.get_download("dl-s").await.unwrap().unwrap();
    assert_eq!(fetched.status, "downloading");
    assert!(fetched.started_at.is_some());
}

#[tokio::test]
async fn test_update_status_to_completed_sets_completed_at() {
    let storage = Storage::open_memory().unwrap();
    let row = make_download("dl-c", "https://example.com/file.zip");
    storage.insert_download(&row).await.unwrap();

    storage
        .update_download_status("dl-c", QueueStatus::Completed)
        .await
        .unwrap();

    let fetched = storage.get_download("dl-c").await.unwrap().unwrap();
    assert_eq!(fetched.status, "completed");
    assert!(fetched.completed_at.is_some());
}

#[tokio::test]
async fn test_update_download_progress() {
    let storage = Storage::open_memory().unwrap();
    let row = make_download("dl-p", "https://example.com/file.zip");
    storage.insert_download(&row).await.unwrap();

    storage
        .update_download_progress("dl-p", 512, 1024)
        .await
        .unwrap();

    let fetched = storage.get_download("dl-p").await.unwrap().unwrap();
    assert_eq!(fetched.bytes_downloaded, 512);
    assert_eq!(fetched.speed_current, 1024);
}

#[tokio::test]
async fn test_update_download_error() {
    let storage = Storage::open_memory().unwrap();
    let row = make_download("dl-e", "https://example.com/file.zip");
    storage.insert_download(&row).await.unwrap();

    storage
        .update_download_error("dl-e", "Connection refused", 2)
        .await
        .unwrap();

    let fetched = storage.get_download("dl-e").await.unwrap().unwrap();
    assert_eq!(fetched.error_message.as_deref(), Some("Connection refused"));
    assert_eq!(fetched.retry_count, 2);
}

#[tokio::test]
async fn test_set_download_priority() {
    let storage = Storage::open_memory().unwrap();
    let row = make_download("dl-prio", "https://example.com/file.zip");
    storage.insert_download(&row).await.unwrap();

    storage.set_download_priority("dl-prio", 10).await.unwrap();

    let fetched = storage.get_download("dl-prio").await.unwrap().unwrap();
    assert_eq!(fetched.priority, 10);
}

// =============================================================================
// List by status and protocol
// =============================================================================

#[tokio::test]
async fn test_list_by_status() {
    let storage = Storage::open_memory().unwrap();

    let dl1 = make_download("dl-q1", "https://example.com/1.zip");
    let dl2 = make_download("dl-q2", "https://example.com/2.zip");
    storage.insert_download(&dl1).await.unwrap();
    storage.insert_download(&dl2).await.unwrap();

    // Mark one as paused
    storage
        .update_download_status("dl-q1", QueueStatus::Paused)
        .await
        .unwrap();

    let queued = storage
        .list_downloads_by_status(QueueStatus::Queued)
        .await
        .unwrap();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].id, "dl-q2");

    let paused = storage
        .list_downloads_by_status(QueueStatus::Paused)
        .await
        .unwrap();
    assert_eq!(paused.len(), 1);
    assert_eq!(paused[0].id, "dl-q1");
}

#[tokio::test]
async fn test_list_by_protocol() {
    let storage = Storage::open_memory().unwrap();

    let dl_http = make_download("dl-http", "https://example.com/1.zip");
    let mut dl_usenet = make_download("dl-usenet", "nzb://example.com/1.nzb");
    dl_usenet.protocol = "usenet".to_string();

    storage.insert_download(&dl_http).await.unwrap();
    storage.insert_download(&dl_usenet).await.unwrap();

    let http_list = storage.list_downloads_by_protocol("http").await.unwrap();
    assert_eq!(http_list.len(), 1);
    assert_eq!(http_list[0].id, "dl-http");

    let usenet_list = storage.list_downloads_by_protocol("usenet").await.unwrap();
    assert_eq!(usenet_list.len(), 1);
    assert_eq!(usenet_list[0].id, "dl-usenet");
}

#[tokio::test]
async fn test_count_by_status() {
    let storage = Storage::open_memory().unwrap();

    for i in 0..5 {
        let dl = make_download(&format!("dl-cnt-{i}"), &format!("https://example.com/{i}.zip"));
        storage.insert_download(&dl).await.unwrap();
    }

    let count = storage.count_by_status(QueueStatus::Queued).await.unwrap();
    assert_eq!(count, 5);

    let count = storage
        .count_by_status(QueueStatus::Completed)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

// =============================================================================
// History
// =============================================================================

#[tokio::test]
async fn test_move_to_history() {
    let storage = Storage::open_memory().unwrap();
    let row = make_download("dl-hist", "https://example.com/file.zip");
    storage.insert_download(&row).await.unwrap();

    // Move to history
    storage.move_to_history("dl-hist").await.unwrap();

    // Download should be gone from active list
    let active = storage.get_download("dl-hist").await.unwrap();
    assert!(active.is_none());

    // Should appear in history
    let history = storage.get_history().await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].id, "dl-hist");
    assert_eq!(history[0].url, "https://example.com/file.zip");
}

#[tokio::test]
async fn test_clear_history() {
    let storage = Storage::open_memory().unwrap();

    // Add and move to history
    for i in 0..3 {
        let dl = make_download(
            &format!("dl-hc-{i}"),
            &format!("https://example.com/{i}.zip"),
        );
        storage.insert_download(&dl).await.unwrap();
        storage.move_to_history(&format!("dl-hc-{i}")).await.unwrap();
    }

    let history = storage.get_history().await.unwrap();
    assert_eq!(history.len(), 3);

    let deleted = storage.clear_history().await.unwrap();
    assert_eq!(deleted, 3);

    let history = storage.get_history().await.unwrap();
    assert!(history.is_empty());
}

// =============================================================================
// Chunks
// =============================================================================

#[tokio::test]
async fn test_chunk_roundtrip() {
    let storage = Storage::open_memory().unwrap();
    let row = make_download("dl-chunk", "https://example.com/big.zip");
    storage.insert_download(&row).await.unwrap();

    let plan = ChunkPlan::split(10_000_000, 4);
    let temp_dir = PathBuf::from("/tmp/test-chunks");
    storage
        .save_chunks("dl-chunk", &plan.chunks, &temp_dir)
        .await
        .unwrap();

    let loaded = storage.load_chunks("dl-chunk").await.unwrap();
    assert_eq!(loaded.len(), 4);
    assert_eq!(loaded[0].start_byte, 0);
    assert_eq!(loaded[3].end_byte, 9_999_999);
}

#[tokio::test]
async fn test_chunk_progress_update() {
    let storage = Storage::open_memory().unwrap();
    let row = make_download("dl-cp", "https://example.com/file.zip");
    storage.insert_download(&row).await.unwrap();

    let plan = ChunkPlan::split(1_000_000, 2);
    let temp_dir = PathBuf::from("/tmp/test-chunks");
    storage
        .save_chunks("dl-cp", &plan.chunks, &temp_dir)
        .await
        .unwrap();

    // Update first chunk to completed
    storage
        .update_chunk_progress("dl-cp", 0, 500_000, "completed")
        .await
        .unwrap();

    let chunks = storage.load_chunks("dl-cp").await.unwrap();
    assert_eq!(chunks[0].bytes_downloaded, 500_000);
    assert_eq!(chunks[0].status, ChunkStatus::Completed);
    // Second chunk should still be pending
    assert_eq!(chunks[1].status, ChunkStatus::Pending);
}

#[tokio::test]
async fn test_delete_chunks() {
    let storage = Storage::open_memory().unwrap();
    let row = make_download("dl-dc", "https://example.com/file.zip");
    storage.insert_download(&row).await.unwrap();

    let plan = ChunkPlan::split(1_000_000, 2);
    let temp_dir = PathBuf::from("/tmp/test-chunks");
    storage
        .save_chunks("dl-dc", &plan.chunks, &temp_dir)
        .await
        .unwrap();

    storage.delete_chunks("dl-dc").await.unwrap();
    let chunks = storage.load_chunks("dl-dc").await.unwrap();
    assert!(chunks.is_empty());
}

#[tokio::test]
async fn test_chunks_cascade_on_download_delete() {
    let storage = Storage::open_memory().unwrap();
    let row = make_download("dl-cascade", "https://example.com/file.zip");
    storage.insert_download(&row).await.unwrap();

    let plan = ChunkPlan::split(1_000_000, 4);
    let temp_dir = PathBuf::from("/tmp/test-chunks");
    storage
        .save_chunks("dl-cascade", &plan.chunks, &temp_dir)
        .await
        .unwrap();

    // Delete the download — chunks should cascade-delete
    storage.delete_download("dl-cascade").await.unwrap();

    let chunks = storage.load_chunks("dl-cascade").await.unwrap();
    assert!(chunks.is_empty());
}

// =============================================================================
// Metadata
// =============================================================================

#[tokio::test]
async fn test_metadata_roundtrip() {
    let storage = Storage::open_memory().unwrap();
    let row = make_download("dl-meta", "https://example.com/file.zip");
    storage.insert_download(&row).await.unwrap();

    let meta = r#"{"nzb_data": "...", "file_count": 3}"#;
    storage
        .update_download_metadata("dl-meta", meta)
        .await
        .unwrap();

    let loaded = storage.get_download_metadata("dl-meta").await.unwrap();
    assert_eq!(loaded.as_deref(), Some(meta));
}

#[tokio::test]
async fn test_metadata_none_for_nonexistent() {
    let storage = Storage::open_memory().unwrap();
    let loaded = storage.get_download_metadata("nope").await.unwrap();
    assert!(loaded.is_none());
}

// =============================================================================
// Update state (key-value store)
// =============================================================================

#[tokio::test]
async fn test_update_state_roundtrip() {
    let storage = Storage::open_memory().unwrap();

    storage
        .set_update_state("test_key", "test_value")
        .await
        .unwrap();

    let val = storage.get_update_state("test_key").await.unwrap();
    assert_eq!(val.as_deref(), Some("test_value"));
}

#[tokio::test]
async fn test_update_state_overwrite() {
    let storage = Storage::open_memory().unwrap();

    storage
        .set_update_state("key", "value1")
        .await
        .unwrap();
    storage
        .set_update_state("key", "value2")
        .await
        .unwrap();

    let val = storage.get_update_state("key").await.unwrap();
    assert_eq!(val.as_deref(), Some("value2"));
}

#[tokio::test]
async fn test_update_state_missing_returns_none() {
    let storage = Storage::open_memory().unwrap();
    let val = storage.get_update_state("missing").await.unwrap();
    assert!(val.is_none());
}

// =============================================================================
// Usenet servers
// =============================================================================

#[tokio::test]
async fn test_usenet_server_crud() {
    let storage = Storage::open_memory().unwrap();

    // Initially empty
    let servers = storage.list_usenet_servers().await.unwrap();
    assert!(servers.is_empty());

    // Insert
    let server = UsenetServerRow {
        id: "srv-1".to_string(),
        name: "Test Server".to_string(),
        host: "news.example.com".to_string(),
        port: 563,
        ssl: true,
        username: "user".to_string(),
        password: "pass".to_string(),
        connections: 10,
        priority: 0,
        created_at: String::new(),
    };
    storage.insert_usenet_server(&server).await.unwrap();

    let servers = storage.list_usenet_servers().await.unwrap();
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].name, "Test Server");
    assert_eq!(servers[0].host, "news.example.com");
    assert!(servers[0].ssl);

    // Delete
    storage.delete_usenet_server("srv-1").await.unwrap();
    let servers = storage.list_usenet_servers().await.unwrap();
    assert!(servers.is_empty());
}

// =============================================================================
// RSS feeds
// =============================================================================

#[tokio::test]
async fn test_rss_feed_crud() {
    let storage = Storage::open_memory().unwrap();

    // Initially empty
    let feeds = storage.list_rss_feeds().await.unwrap();
    assert!(feeds.is_empty());

    // Insert
    let feed = RssFeedRow {
        id: "feed-1".to_string(),
        name: "Test Feed".to_string(),
        url: "https://example.com/rss".to_string(),
        category: "tv".to_string(),
        interval_minutes: 15,
        enabled: true,
        last_check: None,
        last_error: None,
        created_at: String::new(),
    };
    storage.insert_rss_feed(&feed).await.unwrap();

    let feeds = storage.list_rss_feeds().await.unwrap();
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0].name, "Test Feed");
    assert_eq!(feeds[0].category, "tv");
    assert!(feeds[0].enabled);

    // Delete
    storage.delete_rss_feed("feed-1").await.unwrap();
    let feeds = storage.list_rss_feeds().await.unwrap();
    assert!(feeds.is_empty());
}

#[tokio::test]
async fn test_rss_seen_items() {
    let storage = Storage::open_memory().unwrap();

    // Create feed first (foreign key)
    let feed = RssFeedRow {
        id: "feed-seen".to_string(),
        name: "Test".to_string(),
        url: "https://example.com/rss".to_string(),
        category: String::new(),
        interval_minutes: 15,
        enabled: true,
        last_check: None,
        last_error: None,
        created_at: String::new(),
    };
    storage.insert_rss_feed(&feed).await.unwrap();

    // Initially not seen
    let seen = storage
        .is_rss_item_seen("feed-seen", "guid-123")
        .await
        .unwrap();
    assert!(!seen);

    // Mark as seen
    storage
        .mark_rss_item_seen("feed-seen", "guid-123", Some("Episode 1"))
        .await
        .unwrap();

    let seen = storage
        .is_rss_item_seen("feed-seen", "guid-123")
        .await
        .unwrap();
    assert!(seen);

    // Different GUID should not be seen
    let seen = storage
        .is_rss_item_seen("feed-seen", "guid-456")
        .await
        .unwrap();
    assert!(!seen);
}

#[tokio::test]
async fn test_rss_feed_status_update() {
    let storage = Storage::open_memory().unwrap();

    let feed = RssFeedRow {
        id: "feed-status".to_string(),
        name: "Test".to_string(),
        url: "https://example.com/rss".to_string(),
        category: String::new(),
        interval_minutes: 15,
        enabled: true,
        last_check: None,
        last_error: None,
        created_at: String::new(),
    };
    storage.insert_rss_feed(&feed).await.unwrap();

    // Update with error
    storage
        .update_rss_feed_status("feed-status", Some("Connection timeout"))
        .await
        .unwrap();

    let feeds = storage.list_rss_feeds().await.unwrap();
    assert_eq!(feeds[0].last_error.as_deref(), Some("Connection timeout"));
    assert!(feeds[0].last_check.is_some());
}

// =============================================================================
// On-disk storage persistence
// =============================================================================

#[tokio::test]
async fn test_on_disk_persistence() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let dl_dir = dir.path().join("downloads");
    let tmp_dir = dir.path().join("tmp");

    // Create storage, insert data, drop it
    {
        let storage = Storage::open(db_path.clone(), dl_dir.clone(), tmp_dir.clone()).unwrap();
        let row = make_download("persist-1", "https://example.com/persist.zip");
        storage.insert_download(&row).await.unwrap();
    }

    // Reopen and verify data persists
    {
        let storage = Storage::open(db_path, dl_dir, tmp_dir).unwrap();
        let fetched = storage.get_download("persist-1").await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(
            fetched.unwrap().url,
            "https://example.com/persist.zip"
        );
    }
}

// =============================================================================
// Schema / table creation
// =============================================================================

#[tokio::test]
async fn test_fresh_database_has_expected_tables() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("fresh.db");
    let dl_dir = dir.path().join("downloads");
    let tmp_dir = dir.path().join("tmp");

    let storage = Storage::open(db_path, dl_dir, tmp_dir).unwrap();

    // Verify we can query all expected tables without errors
    let _downloads = storage.list_downloads().await.unwrap();
    let _history = storage.get_history().await.unwrap();
    let _servers = storage.list_usenet_servers().await.unwrap();
    let _feeds = storage.list_rss_feeds().await.unwrap();
    let _count = storage.count_by_status(QueueStatus::Queued).await.unwrap();
    let _chunks = storage.load_chunks("nonexistent").await.unwrap();
    let _state = storage.get_update_state("nonexistent").await.unwrap();
}

// =============================================================================
// Bulk operations
// =============================================================================

#[tokio::test]
async fn test_insert_many_downloads() {
    let storage = Storage::open_memory().unwrap();

    for i in 0..100 {
        let dl = make_download(
            &format!("dl-bulk-{i}"),
            &format!("https://example.com/{i}.zip"),
        );
        storage.insert_download(&dl).await.unwrap();
    }

    let list = storage.list_downloads().await.unwrap();
    assert_eq!(list.len(), 100);
}

// =============================================================================
// Concurrent access
// =============================================================================

#[tokio::test]
async fn test_concurrent_writes() {
    let storage = Storage::open_memory().unwrap();

    let mut handles = Vec::new();
    for i in 0..20 {
        let s = storage.clone();
        handles.push(tokio::spawn(async move {
            let dl = make_download(
                &format!("dl-conc-{i}"),
                &format!("https://example.com/{i}.zip"),
            );
            s.insert_download(&dl).await.unwrap();
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let list = storage.list_downloads().await.unwrap();
    assert_eq!(list.len(), 20);
}
