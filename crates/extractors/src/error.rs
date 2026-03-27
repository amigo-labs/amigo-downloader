/// Errors from the extractors crate.
#[derive(Debug, thiserror::Error)]
pub enum ExtractorError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("No streams found: {0}")]
    NoStreams(String),

    #[error("URL not supported: {0}")]
    UnsupportedUrl(String),

    #[error("N-parameter challenge failed: {0}")]
    NChallenge(String),

    #[error("{0}")]
    Other(String),
}
