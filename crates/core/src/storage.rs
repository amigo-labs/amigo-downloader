//! SQLite + filesystem storage abstraction.

use std::path::PathBuf;
use std::sync::Arc;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::queue::QueueStatus;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS downloads (
    id TEXT PRIMARY KEY,
    url TEXT NOT NULL,
    protocol TEXT NOT NULL,
    filename TEXT,
    filesize INTEGER,
    status TEXT NOT NULL DEFAULT 'queued',
    priority INTEGER NOT NULL DEFAULT 0,
    package_id TEXT REFERENCES packages(id),
    plugin_id TEXT,
    download_dir TEXT,
    bytes_downloaded INTEGER DEFAULT 0,
    speed_current INTEGER DEFAULT 0,
    error_message TEXT,
    retry_count INTEGER DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    started_at TEXT,
    completed_at TEXT,
    metadata TEXT
);

CREATE TABLE IF NOT EXISTS chunks (
    id TEXT PRIMARY KEY,
    download_id TEXT NOT NULL REFERENCES downloads(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL,
    start_byte INTEGER NOT NULL,
    end_byte INTEGER NOT NULL,
    bytes_downloaded INTEGER DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'pending',
    temp_path TEXT
);

CREATE TABLE IF NOT EXISTS packages (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'queued',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS history (
    id TEXT PRIMARY KEY,
    url TEXT NOT NULL,
    protocol TEXT NOT NULL,
    filename TEXT,
    filesize INTEGER,
    download_dir TEXT,
    completed_at TEXT NOT NULL DEFAULT (datetime('now')),
    duration_seconds INTEGER,
    avg_speed INTEGER
);

CREATE TABLE IF NOT EXISTS plugin_accounts (
    id TEXT PRIMARY KEY,
    plugin_id TEXT NOT NULL,
    username TEXT NOT NULL,
    password_encrypted TEXT NOT NULL,
    premium_until TEXT,
    is_active INTEGER DEFAULT 1,
    last_login TEXT
);

CREATE TABLE IF NOT EXISTS plugin_storage (
    plugin_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (plugin_id, key)
);

CREATE TABLE IF NOT EXISTS update_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS usenet_servers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    host TEXT NOT NULL,
    port INTEGER NOT NULL DEFAULT 563,
    ssl INTEGER NOT NULL DEFAULT 1,
    username TEXT NOT NULL DEFAULT '',
    password TEXT NOT NULL DEFAULT '',
    connections INTEGER NOT NULL DEFAULT 10,
    priority INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS rss_feeds (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    url TEXT NOT NULL,
    category TEXT NOT NULL DEFAULT '',
    interval_minutes INTEGER NOT NULL DEFAULT 15,
    enabled INTEGER NOT NULL DEFAULT 1,
    last_check TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS rss_seen (
    feed_id TEXT NOT NULL REFERENCES rss_feeds(id) ON DELETE CASCADE,
    guid TEXT NOT NULL,
    title TEXT,
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (feed_id, guid)
);

CREATE INDEX IF NOT EXISTS idx_downloads_status ON downloads(status);
CREATE INDEX IF NOT EXISTS idx_downloads_package ON downloads(package_id);
CREATE INDEX IF NOT EXISTS idx_downloads_protocol ON downloads(protocol);
CREATE INDEX IF NOT EXISTS idx_chunks_download ON chunks(download_id);
CREATE INDEX IF NOT EXISTS idx_history_completed ON history(completed_at);
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRow {
    pub id: String,
    pub url: String,
    pub protocol: String,
    pub filename: Option<String>,
    pub filesize: Option<u64>,
    pub status: String,
    pub priority: i32,
    pub package_id: Option<String>,
    pub plugin_id: Option<String>,
    pub download_dir: Option<String>,
    pub bytes_downloaded: u64,
    pub speed_current: u64,
    pub error_message: Option<String>,
    pub retry_count: u32,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsenetServerRow {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub ssl: bool,
    pub username: String,
    pub password: String,
    pub connections: u32,
    pub priority: u32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RssFeedRow {
    pub id: String,
    pub name: String,
    pub url: String,
    pub category: String,
    pub interval_minutes: u32,
    pub enabled: bool,
    pub last_check: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
}

#[derive(Clone)]
pub struct Storage {
    db: Arc<Mutex<Connection>>,
    pub download_dir: PathBuf,
    pub temp_dir: PathBuf,
}

impl Storage {
    pub fn open(
        db_path: PathBuf,
        download_dir: PathBuf,
        temp_dir: PathBuf,
    ) -> Result<Self, crate::Error> {
        std::fs::create_dir_all(&download_dir)?;
        std::fs::create_dir_all(&temp_dir)?;
        let conn = Connection::open(&db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
            download_dir,
            temp_dir,
        })
    }

    /// Open an in-memory database (for tests).
    pub fn open_memory() -> Result<Self, crate::Error> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
            download_dir: PathBuf::from("/tmp/amigo-downloads"),
            temp_dir: PathBuf::from("/tmp/amigo-tmp"),
        })
    }

    pub async fn insert_download(&self, row: &DownloadRow) -> Result<(), crate::Error> {
        let db = self.db.lock().await;
        db.execute(
            "INSERT INTO downloads (id, url, protocol, filename, filesize, status, priority, package_id, plugin_id, download_dir, bytes_downloaded, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, datetime('now'))",
            rusqlite::params![
                row.id, row.url, row.protocol, row.filename, row.filesize,
                row.status, row.priority, row.package_id, row.plugin_id,
                row.download_dir, row.bytes_downloaded,
            ],
        )?;
        Ok(())
    }

    pub async fn get_download(&self, id: &str) -> Result<Option<DownloadRow>, crate::Error> {
        let db = self.db.lock().await;
        let mut stmt = db.prepare(
            "SELECT id, url, protocol, filename, filesize, status, priority, package_id, plugin_id,
                    download_dir, bytes_downloaded, speed_current, error_message, retry_count,
                    created_at, started_at, completed_at
             FROM downloads WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(rusqlite::params![id], row_to_download)?;
        Ok(rows.next().transpose()?)
    }

    pub async fn list_downloads(&self) -> Result<Vec<DownloadRow>, crate::Error> {
        let db = self.db.lock().await;
        let mut stmt = db.prepare(
            "SELECT id, url, protocol, filename, filesize, status, priority, package_id, plugin_id,
                    download_dir, bytes_downloaded, speed_current, error_message, retry_count,
                    created_at, started_at, completed_at
             FROM downloads ORDER BY priority DESC, created_at ASC",
        )?;
        let rows = stmt.query_map([], row_to_download)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub async fn list_downloads_by_status(
        &self,
        status: QueueStatus,
    ) -> Result<Vec<DownloadRow>, crate::Error> {
        let db = self.db.lock().await;
        let status_str = status.as_str();
        let mut stmt = db.prepare(
            "SELECT id, url, protocol, filename, filesize, status, priority, package_id, plugin_id,
                    download_dir, bytes_downloaded, speed_current, error_message, retry_count,
                    created_at, started_at, completed_at
             FROM downloads WHERE status = ?1 ORDER BY priority DESC, created_at ASC",
        )?;
        let rows = stmt.query_map(rusqlite::params![status_str], row_to_download)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub async fn update_download_status(
        &self,
        id: &str,
        status: QueueStatus,
    ) -> Result<(), crate::Error> {
        let db = self.db.lock().await;
        let status_str = status.as_str();
        match status {
            QueueStatus::Downloading => {
                db.execute(
                    "UPDATE downloads SET status = ?1, started_at = datetime('now') WHERE id = ?2",
                    rusqlite::params![status_str, id],
                )?;
            }
            QueueStatus::Completed => {
                db.execute(
                    "UPDATE downloads SET status = ?1, completed_at = datetime('now') WHERE id = ?2",
                    rusqlite::params![status_str, id],
                )?;
            }
            _ => {
                db.execute(
                    "UPDATE downloads SET status = ?1 WHERE id = ?2",
                    rusqlite::params![status_str, id],
                )?;
            }
        }
        Ok(())
    }

    pub async fn update_download_progress(
        &self,
        id: &str,
        bytes_downloaded: u64,
        speed: u64,
    ) -> Result<(), crate::Error> {
        let db = self.db.lock().await;
        db.execute(
            "UPDATE downloads SET bytes_downloaded = ?1, speed_current = ?2 WHERE id = ?3",
            rusqlite::params![bytes_downloaded, speed, id],
        )?;
        Ok(())
    }

    pub async fn update_download_error(
        &self,
        id: &str,
        error: &str,
        retry_count: u32,
    ) -> Result<(), crate::Error> {
        let db = self.db.lock().await;
        db.execute(
            "UPDATE downloads SET error_message = ?1, retry_count = ?2 WHERE id = ?3",
            rusqlite::params![error, retry_count, id],
        )?;
        Ok(())
    }

    pub async fn update_download_metadata(
        &self,
        id: &str,
        metadata: &str,
    ) -> Result<(), crate::Error> {
        let db = self.db.lock().await;
        db.execute(
            "UPDATE downloads SET metadata = ?1 WHERE id = ?2",
            rusqlite::params![metadata, id],
        )?;
        Ok(())
    }

    pub async fn get_download_metadata(
        &self,
        id: &str,
    ) -> Result<Option<String>, crate::Error> {
        let db = self.db.lock().await;
        let mut stmt = db.prepare("SELECT metadata FROM downloads WHERE id = ?1")?;
        let mut rows = stmt.query_map(rusqlite::params![id], |row| row.get::<_, Option<String>>(0))?;
        Ok(rows.next().transpose()?.flatten())
    }

    pub async fn set_download_priority(
        &self,
        id: &str,
        priority: i32,
    ) -> Result<(), crate::Error> {
        let db = self.db.lock().await;
        db.execute(
            "UPDATE downloads SET priority = ?1 WHERE id = ?2",
            rusqlite::params![priority, id],
        )?;
        Ok(())
    }

    pub async fn delete_download(&self, id: &str) -> Result<(), crate::Error> {
        let db = self.db.lock().await;
        db.execute("DELETE FROM downloads WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    pub async fn move_to_history(&self, id: &str) -> Result<(), crate::Error> {
        let db = self.db.lock().await;
        let tx = db.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO history (id, url, protocol, filename, filesize, download_dir, completed_at)
             SELECT id, url, protocol, filename, filesize, download_dir, datetime('now')
             FROM downloads WHERE id = ?1",
            rusqlite::params![id],
        )?;
        tx.execute("DELETE FROM downloads WHERE id = ?1", rusqlite::params![id])?;
        tx.commit()?;
        Ok(())
    }

    pub async fn get_history(&self) -> Result<Vec<DownloadRow>, crate::Error> {
        let db = self.db.lock().await;
        let mut stmt = db.prepare(
            "SELECT id, url, protocol, filename, filesize, 'completed', 0, NULL, NULL,
                    download_dir, COALESCE(filesize, 0), 0, NULL, 0,
                    completed_at, NULL, completed_at
             FROM history ORDER BY completed_at DESC",
        )?;
        let rows = stmt.query_map([], row_to_download)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub async fn clear_history(&self) -> Result<u64, crate::Error> {
        let db = self.db.lock().await;
        let deleted = db.execute("DELETE FROM history", [])?;
        Ok(deleted as u64)
    }

    pub async fn count_by_status(&self, status: QueueStatus) -> Result<u32, crate::Error> {
        let db = self.db.lock().await;
        let count: u32 = db.query_row(
            "SELECT COUNT(*) FROM downloads WHERE status = ?1",
            rusqlite::params![status.as_str()],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    // --- Chunk persistence for resumable downloads ---

    /// Save a chunk plan for a download (overwrites any existing chunks).
    pub async fn save_chunks(
        &self,
        download_id: &str,
        chunks: &[crate::chunk::Chunk],
        temp_dir: &std::path::Path,
    ) -> Result<(), crate::Error> {
        let db = self.db.lock().await;
        db.execute(
            "DELETE FROM chunks WHERE download_id = ?1",
            rusqlite::params![download_id],
        )?;
        for chunk in chunks {
            let temp_path = temp_dir.join(format!("chunk_{}", chunk.index));
            db.execute(
                "INSERT INTO chunks (id, download_id, chunk_index, start_byte, end_byte, bytes_downloaded, status, temp_path)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    format!("{download_id}_chunk_{}", chunk.index),
                    download_id,
                    chunk.index,
                    chunk.start_byte,
                    chunk.end_byte,
                    chunk.bytes_downloaded,
                    format!("{:?}", chunk.status).to_lowercase(),
                    temp_path.to_string_lossy(),
                ],
            )?;
        }
        Ok(())
    }

    /// Load saved chunks for a download. Returns empty vec if none.
    pub async fn load_chunks(
        &self,
        download_id: &str,
    ) -> Result<Vec<crate::chunk::Chunk>, crate::Error> {
        let db = self.db.lock().await;
        let mut stmt = db.prepare(
            "SELECT chunk_index, start_byte, end_byte, bytes_downloaded, status
             FROM chunks WHERE download_id = ?1 ORDER BY chunk_index",
        )?;
        let chunks = stmt
            .query_map(rusqlite::params![download_id], |row| {
                let status_str: String = row.get(4)?;
                let status = match status_str.as_str() {
                    "completed" => crate::chunk::ChunkStatus::Completed,
                    "downloading" => crate::chunk::ChunkStatus::Pending, // Treat interrupted as pending
                    "failed" => crate::chunk::ChunkStatus::Failed,
                    _ => crate::chunk::ChunkStatus::Pending,
                };
                Ok(crate::chunk::Chunk {
                    index: row.get(0)?,
                    start_byte: row.get(1)?,
                    end_byte: row.get(2)?,
                    bytes_downloaded: row.get(3)?,
                    status,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(chunks)
    }

    /// Update a single chunk's progress.
    pub async fn update_chunk_progress(
        &self,
        download_id: &str,
        chunk_index: u32,
        bytes_downloaded: u64,
        status: &str,
    ) -> Result<(), crate::Error> {
        let db = self.db.lock().await;
        db.execute(
            "UPDATE chunks SET bytes_downloaded = ?1, status = ?2
             WHERE download_id = ?3 AND chunk_index = ?4",
            rusqlite::params![bytes_downloaded, status, download_id, chunk_index],
        )?;
        Ok(())
    }

    /// Delete all chunks for a download (after successful reassembly).
    pub async fn delete_chunks(&self, download_id: &str) -> Result<(), crate::Error> {
        let db = self.db.lock().await;
        db.execute(
            "DELETE FROM chunks WHERE download_id = ?1",
            rusqlite::params![download_id],
        )?;
        Ok(())
    }

    // --- Update state ---

    pub async fn get_update_state(&self, key: &str) -> Result<Option<String>, crate::Error> {
        let db = self.db.lock().await;
        let mut stmt = db.prepare("SELECT value FROM update_state WHERE key = ?1")?;
        let mut rows = stmt.query_map(rusqlite::params![key], |row| row.get::<_, String>(0))?;
        Ok(rows.next().transpose()?)
    }

    pub async fn set_update_state(&self, key: &str, value: &str) -> Result<(), crate::Error> {
        let db = self.db.lock().await;
        db.execute(
            "INSERT OR REPLACE INTO update_state (key, value, updated_at) VALUES (?1, ?2, datetime('now'))",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }

    // --- Usenet servers ---

    pub async fn list_usenet_servers(&self) -> Result<Vec<UsenetServerRow>, crate::Error> {
        let db = self.db.lock().await;
        let mut stmt = db.prepare(
            "SELECT id, name, host, port, ssl, username, password, connections, priority, created_at
             FROM usenet_servers ORDER BY priority ASC, name ASC",
        )?;
        let rows = stmt.query_map([], row_to_usenet_server)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub async fn insert_usenet_server(&self, row: &UsenetServerRow) -> Result<(), crate::Error> {
        let db = self.db.lock().await;
        db.execute(
            "INSERT INTO usenet_servers (id, name, host, port, ssl, username, password, connections, priority)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                row.id, row.name, row.host, row.port as i64, row.ssl as i64,
                row.username, row.password, row.connections as i64, row.priority as i64,
            ],
        )?;
        Ok(())
    }

    pub async fn delete_usenet_server(&self, id: &str) -> Result<(), crate::Error> {
        let db = self.db.lock().await;
        db.execute(
            "DELETE FROM usenet_servers WHERE id = ?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    pub async fn list_downloads_by_protocol(
        &self,
        protocol: &str,
    ) -> Result<Vec<DownloadRow>, crate::Error> {
        let db = self.db.lock().await;
        let mut stmt = db.prepare(
            "SELECT id, url, protocol, filename, filesize, status, priority, package_id, plugin_id,
                    download_dir, bytes_downloaded, speed_current, error_message, retry_count,
                    created_at, started_at, completed_at
             FROM downloads WHERE protocol = ?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(rusqlite::params![protocol], row_to_download)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    // --- RSS feeds ---

    pub async fn list_rss_feeds(&self) -> Result<Vec<RssFeedRow>, crate::Error> {
        let db = self.db.lock().await;
        let mut stmt = db.prepare(
            "SELECT id, name, url, category, interval_minutes, enabled, last_check, last_error, created_at
             FROM rss_feeds ORDER BY name ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(RssFeedRow {
                id: row.get(0)?,
                name: row.get(1)?,
                url: row.get(2)?,
                category: row.get(3)?,
                interval_minutes: row.get::<_, i64>(4)? as u32,
                enabled: row.get::<_, i64>(5)? != 0,
                last_check: row.get(6)?,
                last_error: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub async fn insert_rss_feed(&self, row: &RssFeedRow) -> Result<(), crate::Error> {
        let db = self.db.lock().await;
        db.execute(
            "INSERT INTO rss_feeds (id, name, url, category, interval_minutes, enabled)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                row.id, row.name, row.url, row.category,
                row.interval_minutes as i64, row.enabled as i64,
            ],
        )?;
        Ok(())
    }

    pub async fn delete_rss_feed(&self, id: &str) -> Result<(), crate::Error> {
        let db = self.db.lock().await;
        db.execute("DELETE FROM rss_feeds WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    pub async fn update_rss_feed_status(
        &self,
        id: &str,
        last_error: Option<&str>,
    ) -> Result<(), crate::Error> {
        let db = self.db.lock().await;
        db.execute(
            "UPDATE rss_feeds SET last_check = datetime('now'), last_error = ?1 WHERE id = ?2",
            rusqlite::params![last_error, id],
        )?;
        Ok(())
    }

    pub async fn is_rss_item_seen(&self, feed_id: &str, guid: &str) -> Result<bool, crate::Error> {
        let db = self.db.lock().await;
        let count: u32 = db.query_row(
            "SELECT COUNT(*) FROM rss_seen WHERE feed_id = ?1 AND guid = ?2",
            rusqlite::params![feed_id, guid],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub async fn mark_rss_item_seen(
        &self,
        feed_id: &str,
        guid: &str,
        title: Option<&str>,
    ) -> Result<(), crate::Error> {
        let db = self.db.lock().await;
        db.execute(
            "INSERT OR IGNORE INTO rss_seen (feed_id, guid, title) VALUES (?1, ?2, ?3)",
            rusqlite::params![feed_id, guid, title],
        )?;
        Ok(())
    }
}

fn row_to_usenet_server(row: &rusqlite::Row<'_>) -> rusqlite::Result<UsenetServerRow> {
    Ok(UsenetServerRow {
        id: row.get(0)?,
        name: row.get(1)?,
        host: row.get(2)?,
        port: row.get::<_, i64>(3)? as u16,
        ssl: row.get::<_, i64>(4)? != 0,
        username: row.get(5)?,
        password: row.get(6)?,
        connections: row.get::<_, i64>(7)? as u32,
        priority: row.get::<_, i64>(8)? as u32,
        created_at: row.get(9)?,
    })
}

fn row_to_download(row: &rusqlite::Row<'_>) -> rusqlite::Result<DownloadRow> {
    Ok(DownloadRow {
        id: row.get(0)?,
        url: row.get(1)?,
        protocol: row.get(2)?,
        filename: row.get(3)?,
        filesize: row.get::<_, Option<i64>>(4)?.map(|v| v as u64),
        status: row.get(5)?,
        priority: row.get(6)?,
        package_id: row.get(7)?,
        plugin_id: row.get(8)?,
        download_dir: row.get(9)?,
        bytes_downloaded: row.get::<_, i64>(10)? as u64,
        speed_current: row.get::<_, i64>(11)? as u64,
        error_message: row.get(12)?,
        retry_count: row.get::<_, i32>(13)? as u32,
        created_at: row.get(14)?,
        started_at: row.get(15)?,
        completed_at: row.get(16)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_insert_and_get_download() {
        let storage = Storage::open_memory().unwrap();
        let row = DownloadRow {
            id: "test-1".into(),
            url: "https://example.com/file.zip".into(),
            protocol: "http".into(),
            filename: Some("file.zip".into()),
            filesize: Some(1024),
            status: "queued".into(),
            priority: 0,
            package_id: None,
            plugin_id: None,
            download_dir: None,
            bytes_downloaded: 0,
            speed_current: 0,
            error_message: None,
            retry_count: 0,
            created_at: String::new(),
            started_at: None,
            completed_at: None,
        };
        storage.insert_download(&row).await.unwrap();
        let fetched = storage.get_download("test-1").await.unwrap().unwrap();
        assert_eq!(fetched.url, "https://example.com/file.zip");
        assert_eq!(fetched.filename.as_deref(), Some("file.zip"));
    }

    #[tokio::test]
    async fn test_update_status() {
        let storage = Storage::open_memory().unwrap();
        let row = DownloadRow {
            id: "test-2".into(),
            url: "https://example.com/file.zip".into(),
            protocol: "http".into(),
            filename: None,
            filesize: None,
            status: "queued".into(),
            priority: 0,
            package_id: None,
            plugin_id: None,
            download_dir: None,
            bytes_downloaded: 0,
            speed_current: 0,
            error_message: None,
            retry_count: 0,
            created_at: String::new(),
            started_at: None,
            completed_at: None,
        };
        storage.insert_download(&row).await.unwrap();
        storage
            .update_download_status("test-2", QueueStatus::Downloading)
            .await
            .unwrap();
        let fetched = storage.get_download("test-2").await.unwrap().unwrap();
        assert_eq!(fetched.status, "downloading");
    }

    #[tokio::test]
    async fn test_list_downloads() {
        let storage = Storage::open_memory().unwrap();
        let downloads = storage.list_downloads().await.unwrap();
        assert!(downloads.is_empty());
    }
}
