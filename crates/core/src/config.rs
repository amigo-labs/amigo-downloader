//! Global configuration — TOML file loading and saving.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::bandwidth::BandwidthConfig;
use crate::postprocess::PostProcessConfig;

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsenetProcessingConfig {
    pub par2_repair: bool,
    pub auto_unrar: bool,
    pub delete_archives_after_extract: bool,
    pub delete_par2_after_repair: bool,
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
            info!("Config file not found at {}, using defaults", path.display());
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

    /// Save config to a TOML file.
    pub fn save(&self, path: &Path) -> Result<(), crate::Error> {
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
        assert_eq!(parsed.max_concurrent_downloads, config.max_concurrent_downloads);
        assert_eq!(parsed.download_dir, config.download_dir);
        assert_eq!(parsed.http.max_chunks_per_download, config.http.max_chunks_per_download);
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
    fn test_config_load_missing_file_creates_default() {
        let path = std::env::temp_dir().join("amigo-config-test-missing").join("config.toml");
        let _ = std::fs::remove_file(&path);

        let config = Config::load(&path).unwrap();
        assert_eq!(config.download_dir, "downloads");

        // File should now exist with defaults
        assert!(path.exists());
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }
}
