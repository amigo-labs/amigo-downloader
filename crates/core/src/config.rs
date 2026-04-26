//! Global configuration — TOML file loading and saving.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::bandwidth::BandwidthConfig;
use crate::captcha::CaptchaConfig;
use crate::postprocess::PostProcessConfig;

/// Webhook endpoint configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEndpoint {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default = "default_webhook_events")]
    pub events: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_retry_count")]
    pub retry_count: u32,
    #[serde(default = "default_retry_delay")]
    pub retry_delay_secs: u32,
}

fn default_webhook_events() -> Vec<String> {
    vec!["*".to_string()]
}
fn default_true() -> bool {
    true
}
fn default_retry_count() -> u32 {
    3
}
fn default_retry_delay() -> u32 {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub download_dir: String,
    pub temp_dir: String,
    pub max_concurrent_downloads: u32,
    pub bandwidth: BandwidthConfig,
    pub http: HttpConfig,
    pub usenet: UsenetProcessingConfig,
    pub postprocessing: PostProcessConfig,
    pub update: UpdateConfig,
    pub feedback: FeedbackConfig,
    pub captcha: CaptchaConfig,
    #[serde(default)]
    pub retry: RetryConfig,
    #[serde(default)]
    pub webhooks: Vec<WebhookEndpoint>,
    #[serde(default)]
    pub features: FeatureFlags,
    #[serde(default)]
    pub nzbget_api: NzbGetApiConfig,
    #[serde(default)]
    pub server: ServerConfig,
}

/// HTTP server binding and authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Socket address to bind the REST API + Web UI server to.
    /// Defaults to `127.0.0.1:1516` (local-only). Set to `0.0.0.0:1516` to expose
    /// on the LAN — the first-run setup wizard kicks in automatically.
    pub bind: String,
    /// Pre-shared bearer token for legacy/scripted access. When set, it is
    /// accepted as `Authorization: Bearer <token>` on every `/api/v1/*` route
    /// in addition to the cookie / API-token mechanisms. Leave `None` and
    /// use the device-pairing flow instead.
    #[serde(default)]
    pub api_token: Option<String>,
    /// Explicit CORS allowlist. Empty = no CORS layer (same-origin only, which is the
    /// default because the Web UI is served from the same origin).
    #[serde(default)]
    pub cors_origins: Vec<String>,
    /// `true` once the first-run setup wizard has finished. When `false` and
    /// the bind is non-loopback, the server enters setup-mode: all endpoints
    /// except `/api/v1/setup/*` return 503 with `{"error": "setup_required"}`.
    #[serde(default)]
    pub setup_complete: bool,
    /// Admin username chosen in the setup wizard (or via `AMIGO_SETUP_USER`).
    #[serde(default)]
    pub admin_username: Option<String>,
    /// Argon2id hash of the admin password (PHC-formatted). Never the plaintext.
    #[serde(default)]
    pub admin_password_hash: Option<String>,
    /// Session lifetime in seconds. Defaults to 30 days; refreshed on activity.
    #[serde(default = "default_session_ttl_secs")]
    pub session_ttl_secs: i64,
    /// When `true`, the server trusts `X-Forwarded-For` / `X-Forwarded-Proto`
    /// headers on incoming requests — enable this only when sitting behind a
    /// reverse proxy (nginx, Caddy, Traefik, Cloudflare Tunnel, Tailscale
    /// Funnel) that you control. Off by default to prevent source-IP
    /// spoofing on directly-exposed servers.
    #[serde(default)]
    pub trust_proxy: bool,
}

fn default_session_ttl_secs() -> i64 {
    30 * 24 * 3600
}

impl ServerConfig {
    /// `true` if the bind address is a loopback host (127.0.0.1 / ::1 /
    /// localhost). On loopback we skip setup entirely — a Tauri desktop,
    /// a local `amigo-server` run, or a dev run are trusted by construction.
    pub fn is_bind_loopback(&self) -> bool {
        let host = self
            .bind
            .rsplit_once(':')
            .map(|(h, _)| h)
            .unwrap_or(self.bind.as_str())
            .trim_start_matches('[')
            .trim_end_matches(']');
        matches!(host, "127.0.0.1" | "::1" | "localhost")
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:1516".into(),
            api_token: None,
            cors_origins: Vec::new(),
            setup_complete: false,
            admin_username: None,
            admin_password_hash: None,
            session_ttl_secs: default_session_ttl_secs(),
            trust_proxy: false,
        }
    }
}

/// NZBGet-compatible JSON-RPC API credentials for Sonarr/Radarr integration.
///
/// `enabled` defaults to `false` via `Default`, so a fresh install does not
/// expose the JSON-RPC surface (which can queue/pause/delete downloads)
/// before the operator turns it on and sets credentials.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NzbGetApiConfig {
    /// Enable the NZBGet-compatible JSON-RPC API.
    pub enabled: bool,
    /// Username for HTTP Basic Auth. Both fields must be non-empty when
    /// `enabled` is true; the server refuses to start otherwise (see
    /// `nzbget_api::validate_nzbget_config`).
    pub username: String,
    /// Password for HTTP Basic Auth. Same non-empty requirement as `username`.
    pub password: String,
}

/// Retry behavior for failed downloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts before giving up.
    pub max_retries: u32,
    /// Initial delay before the first retry (seconds).
    pub base_delay_secs: f64,
    /// Maximum delay between retries (seconds). Exponential backoff is capped at this.
    pub max_delay_secs: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            base_delay_secs: 1.0,
            max_delay_secs: 60.0,
        }
    }
}

/// Optional feature toggles — disabled by default, user enables in Settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeatureFlags {
    /// Enable Usenet mode (NZB import, NNTP servers, watch folder).
    pub usenet: bool,
    /// RSS/Atom feed monitoring for automatic NZB import.
    pub rss_feeds: bool,
    /// Show per-server connection statistics in the Usenet UI.
    pub server_stats: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    /// Automatically check for updates periodically.
    pub auto_check: bool,
    /// Hours between automatic update checks.
    pub check_interval_hours: u64,
    /// URL of the plugin registry index.json.
    pub plugin_registry_url: String,
    /// GitHub repository for core releases.
    pub github_repo: String,
    /// Automatically install plugin updates when the periodic check finds
    /// them. Opt-in — defaults to `false` because auto-installing code from
    /// a remote registry is a sensitive operation. Safe to enable only
    /// because the registry index is now Ed25519-signed (audit #31).
    #[serde(default)]
    pub auto_update_plugins: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackConfig {
    /// Enable in-app feedback (requires github_token).
    pub enabled: bool,
    /// GitHub Personal Access Token with `repo` scope.
    /// Can also be set via AMIGO_GITHUB_TOKEN env var.
    pub github_token: String,
    /// Target GitHub repo for issues (owner/repo).
    pub github_repo: String,
    /// Max issues per hour (rate limiting).
    pub max_issues_per_hour: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsenetProcessingConfig {
    pub par2_repair: bool,
    pub auto_unrar: bool,
    pub delete_archives_after_extract: bool,
    pub delete_par2_after_repair: bool,
    /// Selective PAR2: only download recovery volumes when repair is needed.
    /// When false, all PAR2 files (including .vol*.par2) are downloaded upfront.
    #[serde(default = "default_true_fn")]
    pub selective_par2: bool,
    /// Run PAR2 verify/repair and archive extraction sequentially (one after another).
    /// Enable on low-power devices (Raspberry Pi) to reduce CPU/memory pressure.
    /// When false, PAR2 and extraction run in parallel where possible.
    #[serde(default)]
    pub sequential_postprocess: bool,
}

fn default_true_fn() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    pub max_chunks_per_download: u32,
    pub max_connections_per_host: u32,
    pub user_agent: String,
    pub timeout_connect_secs: u64,
    pub timeout_read_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            download_dir: "downloads".into(),
            temp_dir: "downloads/.tmp".into(),
            max_concurrent_downloads: 10,
            bandwidth: BandwidthConfig::default(),
            http: HttpConfig::default(),
            usenet: UsenetProcessingConfig::default(),
            postprocessing: PostProcessConfig::default(),
            update: UpdateConfig::default(),
            feedback: FeedbackConfig::default(),
            captcha: CaptchaConfig::default(),
            retry: RetryConfig::default(),
            webhooks: Vec::new(),
            features: FeatureFlags::default(),
            nzbget_api: NzbGetApiConfig::default(),
            server: ServerConfig::default(),
        }
    }
}

impl Default for FeedbackConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            github_token: std::env::var("AMIGO_GITHUB_TOKEN").unwrap_or_default(),
            github_repo: "amigo-labs/amigo-downloader".into(),
            max_issues_per_hour: 5,
        }
    }
}

impl Default for UsenetProcessingConfig {
    fn default() -> Self {
        Self {
            par2_repair: true,
            auto_unrar: true,
            delete_archives_after_extract: true,
            delete_par2_after_repair: true,
            selective_par2: true,
            sequential_postprocess: false,
        }
    }
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            auto_check: true,
            check_interval_hours: 24,
            plugin_registry_url: "https://raw.githubusercontent.com/amigo-labs/amigo-downloader-plugins/main/index.json".into(),
            github_repo: "amigo-labs/amigo-downloader".into(),
            auto_update_plugins: false,
        }
    }
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            max_chunks_per_download: 8,
            max_connections_per_host: 4,
            user_agent: "amigo-downloader/0.1.0".into(),
            timeout_connect_secs: 30,
            timeout_read_secs: 120,
        }
    }
}

impl Config {
    /// Load config from a TOML file. Falls back to defaults if file doesn't exist.
    pub fn load(path: &Path) -> Result<Self, crate::Error> {
        if !path.exists() {
            info!(
                "Config file not found at {}, using defaults",
                path.display()
            );
            let config = Self::default();
            config.save(path)?;
            return Ok(config);
        }

        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)
            .map_err(|e| crate::Error::Other(format!("Failed to parse config: {e}")))?;
        info!("Loaded config from {}", path.display());
        Ok(config)
    }

    /// Resolve the config file path (first writable standard path).
    pub fn resolve_path() -> std::path::PathBuf {
        if let Ok(dir) = std::env::var("AMIGO_CONFIG_DIR") {
            return std::path::PathBuf::from(dir).join("config.toml");
        }
        std::path::PathBuf::from("config.toml")
    }

    /// Load config, trying standard paths in order:
    /// 1. $AMIGO_CONFIG_DIR/config.toml
    /// 2. ./config.toml
    pub fn load_auto() -> Self {
        let paths = [
            std::env::var("AMIGO_CONFIG_DIR")
                .map(|d| std::path::PathBuf::from(d).join("config.toml"))
                .ok(),
            Some(std::path::PathBuf::from("config.toml")),
        ];

        for path in paths.into_iter().flatten() {
            match Self::load(&path) {
                Ok(config) => return config,
                Err(e) => warn!("Failed to load config from {}: {e}", path.display()),
            }
        }

        Self::default()
    }

    /// Validate config values. Returns a list of problems (empty = valid).
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.max_concurrent_downloads == 0 {
            errors.push("max_concurrent_downloads must be > 0".into());
        }
        if self.download_dir.is_empty() {
            errors.push("download_dir must not be empty".into());
        }
        if self.http.max_chunks_per_download == 0 {
            errors.push("http.max_chunks_per_download must be > 0".into());
        }
        if self.http.timeout_connect_secs == 0 {
            errors.push("http.timeout_connect_secs must be > 0".into());
        }
        if self.retry.max_delay_secs < self.retry.base_delay_secs {
            errors.push("retry.max_delay_secs must be >= retry.base_delay_secs".into());
        }
        if self.server.bind.is_empty() {
            errors.push("server.bind must not be empty".into());
        }
        if self.server.session_ttl_secs <= 0 {
            errors.push("server.session_ttl_secs must be > 0".into());
        }
        // Non-loopback binds are allowed without credentials because the
        // server enters setup-mode on its own; the first-run wizard then
        // collects them. Historical `api_token`-only setups keep working.

        errors
    }

    /// Save config to a TOML file. Validates before saving.
    pub fn save(&self, path: &Path) -> Result<(), crate::Error> {
        let errors = self.validate();
        if !errors.is_empty() {
            return Err(crate::Error::Other(format!(
                "Invalid config: {}",
                errors.join("; ")
            )));
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| crate::Error::Other(format!("Failed to serialize config: {e}")))?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(path, content)?;
        info!("Config saved to {}", path.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_roundtrip_toml() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(
            parsed.max_concurrent_downloads,
            config.max_concurrent_downloads
        );
        assert_eq!(parsed.download_dir, config.download_dir);
        assert_eq!(
            parsed.http.max_chunks_per_download,
            config.http.max_chunks_per_download
        );
        assert_eq!(parsed.update.auto_check, config.update.auto_check);
    }

    #[test]
    fn test_config_save_and_load() {
        let dir = std::env::temp_dir().join("amigo-config-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");

        let config = Config::default();
        config.save(&path).unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.download_dir, "downloads");
        assert_eq!(loaded.max_concurrent_downloads, 10);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn loopback_detection() {
        let mut cfg = ServerConfig::default();
        assert!(cfg.is_bind_loopback(), "default must be loopback");

        cfg.bind = "127.0.0.1:1516".into();
        assert!(cfg.is_bind_loopback());
        cfg.bind = "localhost:1516".into();
        assert!(cfg.is_bind_loopback());
        cfg.bind = "[::1]:1516".into();
        assert!(cfg.is_bind_loopback());

        cfg.bind = "0.0.0.0:1516".into();
        assert!(!cfg.is_bind_loopback());
        cfg.bind = "192.168.1.10:1516".into();
        assert!(!cfg.is_bind_loopback());
    }

    #[test]
    fn non_loopback_without_token_is_valid_now() {
        // Setup-mode replaces the old "refuse to start" rule.
        let mut cfg = Config::default();
        cfg.server.bind = "0.0.0.0:1516".into();
        cfg.server.api_token = None;
        cfg.server.setup_complete = false;
        assert!(cfg.validate().is_empty(), "{:?}", cfg.validate());
    }

    #[test]
    fn test_config_load_missing_file_creates_default() {
        let path = std::env::temp_dir()
            .join("amigo-config-test-missing")
            .join("config.toml");
        let _ = std::fs::remove_file(&path);

        let config = Config::load(&path).unwrap();
        assert_eq!(config.download_dir, "downloads");

        // File should now exist with defaults
        assert!(path.exists());
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }
}
