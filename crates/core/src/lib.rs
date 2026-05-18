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

/// Sanitize a user-supplied download category so it can safely be appended to
/// the configured download directory.
///
/// Categories arrive untrusted from the REST API. Without filtering, values
/// like `"../etc"` or `"/tmp"` let a caller write downloads outside the
/// configured base directory. This helper rejects absolute paths, parent
/// references, drive prefixes, and control characters; the remaining segments
/// are run through [`sanitize_filename`] so per-segment platform rules still
/// apply. Returns `None` for empty input or anything that would escape.
pub fn sanitize_category(input: &str) -> Option<String> {
    use std::path::Component;

    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.contains('\0') {
        return None;
    }

    let normalized = trimmed.replace('\\', "/");
    let path = std::path::Path::new(&normalized);

    let mut segments: Vec<String> = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(seg) => {
                let s = seg.to_string_lossy();
                let safe = sanitize_filename(&s);
                if safe == "download" && s != "download" {
                    // sanitize_filename's empty-fallback fired — segment was
                    // entirely junk, refuse rather than silently rewriting.
                    return None;
                }
                segments.push(safe);
            }
            Component::CurDir => {}
            // ParentDir, RootDir, Prefix all imply path escape on at least
            // one platform — refuse outright.
            _ => return None,
        }
    }

    if segments.is_empty() {
        None
    } else {
        Some(segments.join("/"))
    }
}

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

    /// Insert refused because an active download already exists for this URL.
    /// The inner string is the id of the existing row.
    #[error("URL already in queue: {0}")]
    DuplicateUrl(String),

    #[error("{0}")]
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_category_accepts_normal_value() {
        assert_eq!(sanitize_category("movies"), Some("movies".into()));
        assert_eq!(sanitize_category("  movies  "), Some("movies".into()));
        assert_eq!(
            sanitize_category("Series/Season 1"),
            Some("Series/Season 1".into())
        );
    }

    #[test]
    fn sanitize_category_rejects_traversal() {
        assert_eq!(sanitize_category(".."), None);
        assert_eq!(sanitize_category("../etc"), None);
        assert_eq!(sanitize_category("foo/../bar"), None);
        assert_eq!(sanitize_category("foo/..\\..\\bar"), None);
    }

    #[test]
    fn sanitize_category_rejects_absolute() {
        assert_eq!(sanitize_category("/tmp"), None);
        assert_eq!(sanitize_category("/etc/passwd"), None);
        // Backslash-prefixed (Windows-style root) — normalized to / and rejected.
        assert_eq!(sanitize_category("\\windows"), None);
    }

    #[test]
    fn sanitize_category_rejects_empty_and_nul() {
        assert_eq!(sanitize_category(""), None);
        assert_eq!(sanitize_category("   "), None);
        assert_eq!(sanitize_category("foo\0bar"), None);
    }

    #[test]
    fn sanitize_category_strips_invalid_chars() {
        // Colon is invalid on Windows — sanitize_filename replaces with _.
        assert_eq!(sanitize_category("foo:bar"), Some("foo_bar".into()));
    }
}
