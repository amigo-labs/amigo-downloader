pub mod bandwidth;
pub mod captcha;
pub mod chunk;
pub mod config;
pub mod container;
pub mod coordinator;
pub mod i18n;
pub mod postprocess;
pub mod protocol;
pub mod queue;
pub mod retry;
pub mod storage;
pub mod update_events;
pub mod updater;

/// Sanitize a filename to prevent path traversal and platform-invalid characters.
///
/// Strips directory components, replaces control characters and characters that
/// are invalid on Windows (`<>:"/\|?*`) with `_`, and trims leading/trailing
/// dots and spaces. Returns `"download"` if the result would be empty.
pub fn sanitize_filename(name: &str) -> String {
    // Take only the final path component (handles both / and \)
    let name = name.rsplit(['/', '\\']).next().unwrap_or(name);

    // Replace dangerous and platform-invalid characters
    let name: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();

    // Remove leading/trailing dots and spaces (Windows-invalid)
    let name = name.trim_matches(|c| c == '.' || c == ' ');

    if name.is_empty() {
        return "download".to_string();
    }

    name.to_string()
}

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

    #[error("Update error: {0}")]
    Update(String),

    #[error("Self-update not supported in Docker — pull the new image")]
    DockerSelfUpdateNotSupported,

    #[error("Checksum verification failed")]
    ChecksumMismatch,

    #[error("{0}")]
    Other(String),
}
