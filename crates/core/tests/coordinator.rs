//! Integration tests for core download coordinator lifecycle.

use std::path::PathBuf;

use amigo_core::config::Config;
use amigo_core::coordinator::Coordinator;
use amigo_core::queue::QueueStatus;
use amigo_core::storage::Storage;

fn test_coordinator() -> Coordinator {
    let storage = Storage::open_memory().expect("Failed to open in-memory storage");
    // Set max_concurrent_downloads to 0 to prevent auto-start in tests
    // (avoids network calls and filesystem side effects)
    let mut config = Config::default();
    config.max_concurrent_downloads = 0;
    Coordinator::new(config, storage)
}

#[tokio::test]
async fn test_add_download_creates_entry() {
    let coord = test_coordinator();
    let id = coord
        .add_download("https://example.com/file.zip", None)
        .await
        .unwrap();

    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.url, "https://example.com/file.zip");
    assert_eq!(row.protocol, "http");
    assert_eq!(row.status, "queued");
}

#[tokio::test]
async fn test_add_download_deduplication() {
    let coord = test_coordinator();
    let id1 = coord
        .add_download("https://example.com/file.zip", None)
        .await
        .unwrap();
    let id2 = coord
        .add_download("https://example.com/file.zip", None)
        .await
        .unwrap();

    // Same URL should return the same ID (duplicate detection)
    assert_eq!(id1, id2);
}

#[tokio::test]
async fn test_add_download_with_filename() {
    let coord = test_coordinator();
    let id = coord
        .add_download("https://example.com/dl?id=123", Some("custom.zip".into()))
        .await
        .unwrap();

    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.filename, Some("custom.zip".to_string()));
}

#[tokio::test]
async fn test_add_download_with_priority() {
    let coord = test_coordinator();
    let id = coord
        .add_download_with_options(
            "https://example.com/movie.mp4",
            None,
            None,
            5,
        )
        .await
        .unwrap();

    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.priority, 5);
}

#[tokio::test]
async fn test_protocol_detection_http() {
    let coord = test_coordinator();
    let id = coord
        .add_download("https://example.com/file.zip", None)
        .await
        .unwrap();
    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.protocol, "http");
}

#[tokio::test]
async fn test_protocol_detection_hls() {
    let coord = test_coordinator();
    let id = coord
        .add_download("https://stream.example.com/live.m3u8", None)
        .await
        .unwrap();
    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.protocol, "hls");
}

#[tokio::test]
async fn test_protocol_detection_dash() {
    let coord = test_coordinator();
    let id = coord
        .add_download("https://cdn.example.com/video.mpd", None)
        .await
        .unwrap();
    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.protocol, "dash");
}

#[tokio::test]
async fn test_protocol_detection_usenet() {
    let coord = test_coordinator();
    let id = coord
        .add_download("https://nzbindex.com/file.nzb", None)
        .await
        .unwrap();
    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.protocol, "usenet");
}

#[tokio::test]
async fn test_cancel_removes_download() {
    let coord = test_coordinator();
    let id = coord
        .add_download("https://example.com/file.zip", None)
        .await
        .unwrap();

    coord.cancel(&id).await.unwrap();
    let row = coord.storage().get_download(&id).await.unwrap();
    assert!(row.is_none());
}

#[tokio::test]
async fn test_pause_and_resume() {
    let coord = test_coordinator();
    let id = coord
        .add_download("https://example.com/file.zip", None)
        .await
        .unwrap();

    coord.pause(&id).await.unwrap();
    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.status, "paused");

    coord.resume(&id).await.unwrap();
    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    // After resume, status goes back to queued (waiting for a slot)
    assert_eq!(row.status, "queued");
}

#[tokio::test]
async fn test_event_subscription() {
    let coord = test_coordinator();
    let mut rx = coord.subscribe();

    let _id = coord
        .add_download("https://example.com/file.zip", None)
        .await
        .unwrap();

    // Should receive an "Added" event
    let event = rx.recv().await.unwrap();
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["type"], "added");
}

#[tokio::test]
async fn test_recover_downloads_requeues_interrupted() {
    let coord = test_coordinator();
    let id = coord
        .add_download("https://example.com/file.zip", None)
        .await
        .unwrap();

    // Manually set status to "downloading" (simulating crash mid-download)
    coord
        .storage()
        .update_download_status(&id, QueueStatus::Downloading)
        .await
        .unwrap();

    let recovered = coord.recover_downloads().await.unwrap();
    assert_eq!(recovered, 1);

    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.status, "queued");
}

#[tokio::test]
async fn test_chunk_storage_roundtrip() {
    let storage = Storage::open_memory().unwrap();

    // Create a download first
    let row = amigo_core::storage::DownloadRow {
        id: "test-dl".to_string(),
        url: "https://example.com/big.zip".to_string(),
        protocol: "http".to_string(),
        filename: Some("big.zip".to_string()),
        filesize: Some(10_000_000),
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
    };
    storage.insert_download(&row).await.unwrap();

    // Save chunks
    let chunks = amigo_core::chunk::ChunkPlan::split(10_000_000, 4);
    let temp_dir = PathBuf::from("/tmp/test-chunks");
    storage
        .save_chunks("test-dl", &chunks.chunks, &temp_dir)
        .await
        .unwrap();

    // Load them back
    let loaded = storage.load_chunks("test-dl").await.unwrap();
    assert_eq!(loaded.len(), 4);
    assert_eq!(loaded[0].start_byte, 0);
    assert_eq!(loaded[3].end_byte, 9_999_999);

    // Update progress
    storage
        .update_chunk_progress("test-dl", 0, 2_500_000, "completed")
        .await
        .unwrap();

    let reloaded = storage.load_chunks("test-dl").await.unwrap();
    assert_eq!(reloaded[0].bytes_downloaded, 2_500_000);
    assert_eq!(
        reloaded[0].status,
        amigo_core::chunk::ChunkStatus::Completed
    );

    // Delete
    storage.delete_chunks("test-dl").await.unwrap();
    let empty = storage.load_chunks("test-dl").await.unwrap();
    assert!(empty.is_empty());
}

#[tokio::test]
async fn test_config_validation() {
    let mut config = Config::default();
    assert!(config.validate().is_empty());

    config.max_concurrent_downloads = 0;
    let errors = config.validate();
    assert!(!errors.is_empty());
    assert!(errors[0].contains("max_concurrent_downloads"));
}
