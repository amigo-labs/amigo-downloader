//! Background tasks: NZB watch folder monitoring and RSS feed polling.

use std::path::Path;
use std::sync::Arc;

use amigo_core::coordinator::Coordinator;
use tokio::time::{Duration, interval};
use tracing::{debug, info, warn};

/// Start all background tasks.
pub fn spawn_background_tasks(coordinator: Arc<Coordinator>, http_client: reqwest::Client) {
    // NZB watch folder — check every 10 seconds
    let coord = coordinator.clone();
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(10));
        loop {
            ticker.tick().await;
            if let Err(e) = check_nzb_watch_folder(&coord).await {
                debug!("NZB watch folder check: {e}");
            }
        }
    });

    // RSS feed poller — check every 60 seconds (individual feed intervals checked inside)
    let coord = coordinator.clone();
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(60));
        loop {
            ticker.tick().await;
            if let Err(e) = poll_rss_feeds(&coord, &http_client).await {
                debug!("RSS poll cycle: {e}");
            }
        }
    });
}

/// Check the NZB watch folder for new .nzb files and import them.
async fn check_nzb_watch_folder(coordinator: &Arc<Coordinator>) -> Result<(), amigo_core::Error> {
    let watch_dir = coordinator
        .storage()
        .get_update_state("nzb_watch_dir")
        .await?
        .unwrap_or_default();

    if watch_dir.is_empty() {
        return Ok(());
    }

    let dir = Path::new(&watch_dir);
    if !dir.exists() {
        return Ok(());
    }

    let mut entries = tokio::fs::read_dir(dir).await?;
    let processed_dir = dir.join(".processed");

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !name.ends_with(".nzb") {
            continue;
        }

        info!("NZB watch folder: importing {:?}", path);

        match tokio::fs::read_to_string(&path).await {
            Ok(nzb_data) => {
                // Validate NZB
                if amigo_core::protocol::usenet::nzb::parse_nzb(&nzb_data).is_ok() {
                    let filename = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("nzb-import")
                        .to_string();

                    match coordinator
                        .add_download("nzb://watch-folder", Some(filename.clone()))
                        .await
                    {
                        Ok(id) => {
                            // Store NZB metadata
                            let nzb = amigo_core::protocol::usenet::nzb::parse_nzb(&nzb_data).unwrap();
                            let metadata = serde_json::json!({
                                "file_count": nzb.files.len(),
                                "total_bytes": nzb.files.iter().map(|f| f.total_bytes()).sum::<u64>(),
                                "nzb_data": nzb_data,
                            });
                            let _ = coordinator
                                .storage()
                                .update_download_metadata(&id, &metadata.to_string())
                                .await;

                            info!("NZB imported from watch folder: {filename} → {id}");

                            // Move to processed subfolder
                            tokio::fs::create_dir_all(&processed_dir).await.ok();
                            let dest = processed_dir.join(entry.file_name());
                            if let Err(e) = tokio::fs::rename(&path, &dest).await {
                                // If rename fails (cross-device), copy + delete
                                if tokio::fs::copy(&path, &dest).await.is_ok() {
                                    let _ = tokio::fs::remove_file(&path).await;
                                } else {
                                    warn!("Failed to move processed NZB: {e}");
                                }
                            }
                        }
                        Err(e) => warn!("Failed to add NZB from watch folder: {e}"),
                    }
                } else {
                    warn!("Invalid NZB in watch folder: {:?}", path);
                }
            }
            Err(e) => warn!("Failed to read NZB from watch folder: {e}"),
        }
    }

    Ok(())
}

/// Poll all enabled RSS feeds and import new NZB links.
async fn poll_rss_feeds(
    coordinator: &Arc<Coordinator>,
    http_client: &reqwest::Client,
) -> Result<(), amigo_core::Error> {
    // Check if RSS feature is enabled
    let flags = coordinator
        .storage()
        .get_update_state("feature_flags")
        .await?
        .unwrap_or_default();

    let rss_enabled = serde_json::from_str::<serde_json::Value>(&flags)
        .ok()
        .and_then(|v| v.get("rss_feeds")?.as_bool())
        .unwrap_or(false);

    if !rss_enabled {
        return Ok(());
    }

    let feeds = coordinator.storage().list_rss_feeds().await?;

    for feed in &feeds {
        if !feed.enabled {
            continue;
        }

        // Check if enough time has passed since last check
        if let Some(ref last_check) = feed.last_check
            && let Ok(last) = chrono::NaiveDateTime::parse_from_str(last_check, "%Y-%m-%d %H:%M:%S")
        {
            let now = chrono::Utc::now().naive_utc();
            let elapsed = now - last;
            if elapsed.num_minutes() < feed.interval_minutes as i64 {
                continue;
            }
        }

        debug!("Polling RSS feed: {} ({})", feed.name, feed.url);

        match fetch_and_process_feed(coordinator, http_client, feed).await {
            Ok(count) => {
                if count > 0 {
                    info!("RSS feed '{}': imported {count} new items", feed.name);
                }
                coordinator
                    .storage()
                    .update_rss_feed_status(&feed.id, None)
                    .await?;
            }
            Err(e) => {
                warn!("RSS feed '{}' error: {e}", feed.name);
                coordinator
                    .storage()
                    .update_rss_feed_status(&feed.id, Some(&e.to_string()))
                    .await?;
            }
        }
    }

    Ok(())
}

/// Fetch an RSS/Atom feed and process new items.
async fn fetch_and_process_feed(
    coordinator: &Arc<Coordinator>,
    http_client: &reqwest::Client,
    feed: &amigo_core::storage::RssFeedRow,
) -> Result<u32, amigo_core::Error> {
    let resp = http_client
        .get(&feed.url)
        .header("User-Agent", "amigo-downloader/0.1.0")
        .send()
        .await?;

    let body = resp.text().await?;
    let mut imported = 0;

    // Simple RSS/Atom parsing: find <link> or <enclosure> with .nzb URLs
    // This is a lightweight approach; a full XML parser could be added later
    for url in extract_nzb_links(&body) {
        let guid = &url; // Use URL as GUID if no explicit GUID

        if coordinator
            .storage()
            .is_rss_item_seen(&feed.id, guid)
            .await?
        {
            continue;
        }

        // Add the NZB URL as a download
        let category = if feed.category.is_empty() {
            None
        } else {
            Some(feed.category.clone())
        };

        match coordinator
            .add_download_with_options(&url, None, category, 0)
            .await
        {
            Ok(_id) => {
                coordinator
                    .storage()
                    .mark_rss_item_seen(&feed.id, guid, None)
                    .await?;
                imported += 1;
            }
            Err(e) => warn!("RSS: failed to add {url}: {e}"),
        }
    }

    Ok(imported)
}

/// Extract NZB download links from RSS/Atom XML content.
fn extract_nzb_links(xml: &str) -> Vec<String> {
    let mut links = Vec::new();

    // Find enclosure URLs with .nzb
    let enclosure_pattern = regex::Regex::new(
        r#"<enclosure[^>]+url=["']([^"']+\.nzb[^"']*)["']"#,
    )
    .unwrap();
    for cap in enclosure_pattern.captures_iter(xml) {
        if let Some(url) = cap.get(1) {
            links.push(url.as_str().to_string());
        }
    }

    // Find <link> elements pointing to .nzb files
    let link_pattern =
        regex::Regex::new(r#"<link[^>]*>([^<]+\.nzb[^<]*)</link>"#).unwrap();
    for cap in link_pattern.captures_iter(xml) {
        if let Some(url) = cap.get(1) {
            let url = url.as_str().trim();
            if url.starts_with("http") {
                links.push(url.to_string());
            }
        }
    }

    // Find href attributes pointing to .nzb
    let href_pattern =
        regex::Regex::new(r#"href=["']([^"']+\.nzb[^"']*)["']"#).unwrap();
    for cap in href_pattern.captures_iter(xml) {
        if let Some(url) = cap.get(1) {
            links.push(url.as_str().to_string());
        }
    }

    // Deduplicate
    links.sort();
    links.dedup();
    links
}
