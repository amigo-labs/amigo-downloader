//! Global configuration.

use serde::{Deserialize, Serialize};

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
