pub mod bandwidth;
pub mod chunk;
pub mod config;
pub mod container;
pub mod coordinator;
pub mod postprocess;
pub mod protocol;
pub mod queue;
pub mod retry;
pub mod storage;

/// Core error type.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}
