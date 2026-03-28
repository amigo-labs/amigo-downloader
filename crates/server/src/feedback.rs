//! In-app feedback: bug reports, feature requests, crash reports → GitHub Issues.
//!
//! Uses a server-configured GitHub PAT to create issues. No GitHub account
//! needed for end users. Rate-limited to prevent abuse.

use std::sync::Arc;
use std::time::Instant;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use amigo_core::queue::QueueStatus;

use crate::api::AppState;

// --- Rate limiter ---

pub struct RateLimiter {
    timestamps: Vec<Instant>,
    max_per_hour: u32,
}

impl RateLimiter {
    fn new(max_per_hour: u32) -> Self {
        Self {
            timestamps: Vec::new(),
            max_per_hour,
        }
    }

    fn check(&mut self) -> bool {
        let cutoff = Instant::now() - std::time::Duration::from_secs(3600);
        self.timestamps.retain(|t| *t > cutoff);
        if self.timestamps.len() as u32 >= self.max_per_hour {
            return false;
        }
        self.timestamps.push(Instant::now());
        true
    }
}

// --- Types ---

#[derive(Deserialize)]
pub struct FeedbackRequest {
    #[serde(rename = "type")]
    feedback_type: FeedbackType,
    title: String,
    description: String,
    include_system_info: Option<bool>,
    error_context: Option<ErrorContext>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum FeedbackType {
    Bug,
    Feature,
    Crash,
}

#[derive(Deserialize, Serialize, Clone)]
struct ErrorContext {
    download_id: Option<String>,
    error_message: Option<String>,
    url: Option<String>,
}

#[derive(Serialize)]
struct FeedbackResponse {
    issue_number: u64,
    issue_url: String,
    deduplicated: bool,
}

#[derive(Serialize)]
pub struct SystemInfoResponse {
    version: String,
    os: String,
    arch: String,
    active_downloads: usize,
    queued_downloads: u32,
    total_completed: u32,
    plugins_loaded: usize,
    config: ConfigSummary,
    feedback_enabled: bool,
}

#[derive(Serialize)]
struct ConfigSummary {
    max_concurrent_downloads: u32,
    bandwidth_limit: u64,
    http_chunks: u32,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

// GitHub API types
#[derive(Serialize)]
struct GitHubCreateIssue {
    title: String,
    body: String,
    labels: Vec<String>,
}

#[derive(Deserialize)]
struct GitHubIssueResponse {
    number: u64,
    html_url: String,
}

// --- Router ---

pub fn feedback_router(state: AppState, rate_limiter: Arc<Mutex<RateLimiter>>) -> Router {
    Router::new()
        .route("/api/v1/feedback", post(submit_feedback))
        .route("/api/v1/system-info", get(system_info))
        .with_state((state, rate_limiter))
}

pub fn new_rate_limiter(max_per_hour: u32) -> Arc<Mutex<RateLimiter>> {
    Arc::new(Mutex::new(RateLimiter::new(max_per_hour)))
}

type FeedbackState = (AppState, Arc<Mutex<RateLimiter>>);

// --- Handlers ---

async fn system_info(State((state, _)): State<FeedbackState>) -> Json<SystemInfoResponse> {
    let config = state.coordinator.config();
    let active = state.coordinator.active_count().await;
    let queued = state
        .coordinator
        .storage()
        .count_by_status(QueueStatus::Queued)
        .await
        .unwrap_or(0);
    let completed = state
        .coordinator
        .storage()
        .count_by_status(QueueStatus::Completed)
        .await
        .unwrap_or(0);
    let plugins = state.plugins.list_plugins().await;

    Json(SystemInfoResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        active_downloads: active,
        queued_downloads: queued,
        total_completed: completed,
        plugins_loaded: plugins.len(),
        config: ConfigSummary {
            max_concurrent_downloads: config.max_concurrent_downloads,
            bandwidth_limit: config.bandwidth.global_limit,
            http_chunks: config.http.max_chunks_per_download,
        },
        feedback_enabled: !config.feedback.github_token.is_empty(),
    })
}

async fn submit_feedback(
    State((state, rate_limiter)): State<FeedbackState>,
    Json(req): Json<FeedbackRequest>,
) -> Result<(StatusCode, Json<FeedbackResponse>), (StatusCode, Json<ErrorResponse>)> {
    let config = state.coordinator.config();

    // Check if feedback is configured
    let token = if !config.feedback.github_token.is_empty() {
        &config.feedback.github_token
    } else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "Feedback not configured. Set AMIGO_GITHUB_TOKEN or configure feedback.github_token.".into(),
            }),
        ));
    };

    // Rate limit
    {
        let mut limiter = rate_limiter.lock().await;
        if !limiter.check() {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                Json(ErrorResponse {
                    error: format!(
                        "Rate limit exceeded. Max {} issues per hour.",
                        config.feedback.max_issues_per_hour
                    ),
                }),
            ));
        }
    }

    // Validate
    let title = req.title.trim();
    if title.is_empty() || title.len() > 200 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Title must be 1-200 characters.".into(),
            }),
        ));
    }

    // Build issue body
    let mut body = format!("## Description\n\n{}\n", req.description);

    // Add system info if requested
    if req.include_system_info.unwrap_or(true) {
        let active = state.coordinator.active_count().await;
        let plugins = state.plugins.list_plugins().await;
        let plugin_names: Vec<&str> = plugins.iter().map(|p| p.name.as_str()).collect();

        body.push_str(&format!(
            "\n## System Info\n\n\
            | Key | Value |\n\
            |-----|-------|\n\
            | Version | {} |\n\
            | OS | {} ({}) |\n\
            | Active Downloads | {} |\n\
            | Max Concurrent | {} |\n\
            | Bandwidth Limit | {} |\n\
            | Plugins | {} |\n",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            active,
            config.max_concurrent_downloads,
            if config.bandwidth.global_limit == 0 {
                "unlimited".into()
            } else {
                format!("{} B/s", config.bandwidth.global_limit)
            },
            if plugin_names.is_empty() {
                "none".into()
            } else {
                plugin_names.join(", ")
            },
        ));
    }

    // Add error context for crash reports
    if let Some(ref ctx) = req.error_context {
        body.push_str("\n## Error Context\n\n");
        if let Some(ref id) = ctx.download_id {
            body.push_str(&format!("- **Download ID:** `{id}`\n"));
        }
        if let Some(ref err) = ctx.error_message {
            body.push_str(&format!("- **Error:** {err}\n"));
        }
        // Note: We intentionally don't include the URL to avoid leaking private links
    }

    body.push_str("\n---\n*Submitted via amigo-downloader in-app feedback*\n");

    // Labels
    let labels = match req.feedback_type {
        FeedbackType::Bug => vec!["bug".to_string()],
        FeedbackType::Feature => vec!["enhancement".to_string()],
        FeedbackType::Crash => vec!["bug".to_string(), "crash".to_string()],
    };

    let type_prefix = match req.feedback_type {
        FeedbackType::Bug => "[Bug]",
        FeedbackType::Feature => "[Feature]",
        FeedbackType::Crash => "[Crash]",
    };

    // Deduplication: hash error_message + host (not full URL) + version
    // Same error from same hoster = same bug, regardless of specific file
    let dedup_hash = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        if let Some(ref ctx) = req.error_context {
            ctx.error_message.as_deref().unwrap_or("").hash(&mut hasher);
            // Only hash the host part of the URL, not the full path
            if let Some(ref url) = ctx.url {
                let host = url
                    .split("//")
                    .nth(1)
                    .and_then(|s| s.split('/').next())
                    .unwrap_or("");
                host.hash(&mut hasher);
            }
        }
        title.hash(&mut hasher);
        env!("CARGO_PKG_VERSION").hash(&mut hasher);
        format!("amigo-dedup-{:016x}", hasher.finish())
    };

    let repo = &config.feedback.github_repo;

    // Check if an issue with this dedup hash already exists
    if matches!(req.feedback_type, FeedbackType::Crash) {
        match check_duplicate_issue(&state.http_client, token, repo, &dedup_hash).await {
            Ok(Some(existing)) => {
                info!("Duplicate crash found: #{} — skipping", existing.number);
                return Ok((
                    StatusCode::OK,
                    Json(FeedbackResponse {
                        issue_number: existing.number,
                        issue_url: existing.html_url,
                        deduplicated: true,
                    }),
                ));
            }
            Ok(None) => {} // No duplicate, proceed
            Err(e) => {
                warn!("Dedup check failed (proceeding anyway): {e}");
            }
        }
    }

    // Append dedup hash to body (hidden, for future searches)
    body.push_str(&format!("\n<!-- {dedup_hash} -->\n"));

    let issue = GitHubCreateIssue {
        title: format!("{type_prefix} {title}"),
        body,
        labels,
    };

    // Create GitHub issue
    let url = format!("https://api.github.com/repos/{repo}/issues");

    let resp = state
        .http_client
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "amigo-downloader")
        .json(&issue)
        .send()
        .await
        .map_err(|e| {
            error!("GitHub API request failed: {e}");
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    error: format!("GitHub API error: {e}"),
                }),
            )
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        error!("GitHub API returned {status}: {body}");
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("GitHub API returned {status}"),
            }),
        ));
    }

    let gh_issue: GitHubIssueResponse = resp.json().await.map_err(|e| {
        error!("Failed to parse GitHub response: {e}");
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: "Failed to parse GitHub response".into(),
            }),
        )
    })?;

    info!(
        "Created GitHub issue #{}: {}",
        gh_issue.number, gh_issue.html_url
    );

    Ok((
        StatusCode::CREATED,
        Json(FeedbackResponse {
            issue_number: gh_issue.number,
            issue_url: gh_issue.html_url,
            deduplicated: false,
        }),
    ))
}

/// Search GitHub for an existing issue containing the dedup hash.
async fn check_duplicate_issue(
    client: &reqwest::Client,
    token: &str,
    repo: &str,
    dedup_hash: &str,
) -> Result<Option<GitHubIssueResponse>, String> {
    let query = format!("{dedup_hash} repo:{repo} is:issue");
    let url = format!(
        "https://api.github.com/search/issues?q={}&per_page=1",
        urlencoding(&query)
    );

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "amigo-downloader")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("GitHub search API returned {}", resp.status()));
    }

    #[derive(Deserialize)]
    struct SearchResult {
        total_count: u64,
        items: Vec<GitHubIssueResponse>,
    }

    let result: SearchResult = resp.json().await.map_err(|e| e.to_string())?;

    if result.total_count > 0 {
        Ok(result.items.into_iter().next())
    } else {
        Ok(None)
    }
}

/// Simple URL encoding for query parameters.
fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => "+".to_string(),
            '&' | '=' | '?' | '#' | '+' | ':' | '/' => format!("%{:02X}", c as u8),
            _ if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' => {
                c.to_string()
            }
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter_allows_within_limit() {
        let mut limiter = RateLimiter::new(3);
        assert!(limiter.check());
        assert!(limiter.check());
        assert!(limiter.check());
        assert!(!limiter.check()); // 4th should fail
    }

    #[test]
    fn test_feedback_type_deserialization() {
        let json = r#"{"type":"bug","title":"test","description":"desc"}"#;
        let req: FeedbackRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req.feedback_type, FeedbackType::Bug));

        let json = r#"{"type":"feature","title":"test","description":"desc"}"#;
        let req: FeedbackRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req.feedback_type, FeedbackType::Feature));

        let json = r#"{"type":"crash","title":"test","description":"desc","error_context":{"download_id":"abc","error_message":"timeout"}}"#;
        let req: FeedbackRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req.feedback_type, FeedbackType::Crash));
        assert_eq!(req.error_context.unwrap().download_id.unwrap(), "abc");
    }

    #[test]
    fn test_dedup_hash_same_error_same_host() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        fn make_hash(error: &str, url: &str, title: &str) -> String {
            let mut hasher = DefaultHasher::new();
            error.hash(&mut hasher);
            let host = url
                .split("//")
                .nth(1)
                .and_then(|s| s.split('/').next())
                .unwrap_or("");
            host.hash(&mut hasher);
            title.hash(&mut hasher);
            "0.1.0".hash(&mut hasher);
            format!("amigo-dedup-{:016x}", hasher.finish())
        }

        // Same error + same host = same hash
        let h1 = make_hash("timeout", "https://example.com/file1.zip", "timeout");
        let h2 = make_hash("timeout", "https://example.com/file2.zip", "timeout");
        assert_eq!(h1, h2, "Same error on same host should produce same hash");

        // Same error + different host = different hash
        let h3 = make_hash("timeout", "https://other.com/file1.zip", "timeout");
        assert_ne!(
            h1, h3,
            "Same error on different host should produce different hash"
        );

        // Different error + same host = different hash
        let h4 = make_hash(
            "connection reset",
            "https://example.com/file1.zip",
            "connection reset",
        );
        assert_ne!(
            h1, h4,
            "Different error on same host should produce different hash"
        );
    }
}
