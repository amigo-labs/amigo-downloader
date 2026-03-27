//! Post-processing pipeline: extraction, verification, cleanup.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostProcessConfig {
    pub auto_extract: bool,
    pub delete_archives: bool,
    pub verify_checksums: bool,
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self {
            auto_extract: true,
            delete_archives: true,
            verify_checksums: true,
        }
    }
}

pub async fn run_pipeline(_download_path: &str, _config: &PostProcessConfig) -> Result<(), crate::Error> {
    todo!("Run post-processing pipeline: extract, verify, cleanup")
}
