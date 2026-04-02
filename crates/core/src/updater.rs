//! Core binary self-update via GitHub Releases.
//!
//! Checks for new releases, downloads the correct binary for the current
//! platform, verifies SHA256, and performs an atomic self-replace.

use std::path::PathBuf;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use tracing::{debug, info};

/// Current app version from Cargo.toml.
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// How the app is distributed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Distribution {
    Cli,
    Server,
    Docker,
    Tauri,
}

/// Information about a GitHub release.
#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

/// Result of checking for updates.
#[derive(Debug, Clone)]
pub enum CoreUpdateStatus {
    UpToDate,
    UpdateAvailable {
        current: String,
        latest: String,
        release_notes: String,
        can_self_update: bool,
        download_url: String,
        sha256_url: Option<String>,
    },
}

/// Detect how the binary is distributed.
pub fn detect_distribution() -> Distribution {
    if std::env::var("AMIGO_DOCKER").is_ok() {
        return Distribution::Docker;
    }
    if std::env::var("AMIGO_TAURI").is_ok() {
        return Distribution::Tauri;
    }
    Distribution::Server
}

/// Check GitHub for the latest release.
pub async fn check_for_update(
    client: &reqwest::Client,
    github_repo: &str,
) -> Result<CoreUpdateStatus, crate::Error> {
    let url = format!("https://api.github.com/repos/{github_repo}/releases/latest");
    debug!("Checking for core update: {url}");

    let resp = client
        .get(&url)
        .header("User-Agent", "amigo-downloader")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| crate::Error::Update(format!("Failed to check for updates: {e}")))?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        debug!("No releases found");
        return Ok(CoreUpdateStatus::UpToDate);
    }

    if !resp.status().is_success() {
        return Err(crate::Error::Update(format!(
            "GitHub API returned {}",
            resp.status()
        )));
    }

    let release: ReleaseInfo = resp
        .json()
        .await
        .map_err(|e| crate::Error::Update(format!("Failed to parse release: {e}")))?;

    let latest_tag = release.tag_name.trim_start_matches('v');

    let current =
        semver::Version::parse(CURRENT_VERSION).unwrap_or_else(|_| semver::Version::new(0, 0, 0));
    let latest = match semver::Version::parse(latest_tag) {
        Ok(v) => v,
        Err(_) => {
            debug!("Could not parse release tag as semver: {latest_tag}");
            return Ok(CoreUpdateStatus::UpToDate);
        }
    };

    if latest > current {
        let dist = detect_distribution();
        let can_self_update = dist == Distribution::Cli || dist == Distribution::Server;

        let asset = select_asset(&release);
        let download_url = asset
            .map(|a| a.browser_download_url.clone())
            .unwrap_or_default();
        let sha256_url = asset.and_then(|a| {
            let sha_name = format!("{}.sha256", a.name);
            release
                .assets
                .iter()
                .find(|x| x.name == sha_name)
                .map(|x| x.browser_download_url.clone())
        });

        info!("Update available: {CURRENT_VERSION} → {latest_tag}");
        Ok(CoreUpdateStatus::UpdateAvailable {
            current: CURRENT_VERSION.to_string(),
            latest: latest_tag.to_string(),
            release_notes: release.body.unwrap_or_default(),
            can_self_update,
            download_url,
            sha256_url,
        })
    } else {
        debug!("Up to date: {CURRENT_VERSION}");
        Ok(CoreUpdateStatus::UpToDate)
    }
}

/// Select the correct asset for the current platform.
pub fn select_asset(release: &ReleaseInfo) -> Option<&ReleaseAsset> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let target = format!("amigo-server-{os}-{arch}");
    debug!("Looking for asset matching: {target}");

    release
        .assets
        .iter()
        .find(|a| a.name.starts_with(&target) && !a.name.ends_with(".sha256"))
}

/// Download a release asset and verify its SHA256 checksum.
pub async fn download_and_verify(
    client: &reqwest::Client,
    asset: &ReleaseAsset,
    sha256_url: Option<&str>,
) -> Result<PathBuf, crate::Error> {
    info!("Downloading update: {} ({} bytes)", asset.name, asset.size);

    let resp = client
        .get(&asset.browser_download_url)
        .send()
        .await
        .map_err(|e| crate::Error::Update(format!("Download failed: {e}")))?
        .error_for_status()
        .map_err(|e| crate::Error::Update(format!("Download failed: {e}")))?;

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| crate::Error::Update(format!("Download failed: {e}")))?;

    // Verify SHA256 if checksum URL provided
    if let Some(sha_url) = sha256_url {
        let expected = client
            .get(sha_url)
            .send()
            .await
            .map_err(|e| crate::Error::Update(format!("Checksum download failed: {e}")))?
            .text()
            .await
            .map_err(|e| crate::Error::Update(format!("Checksum read failed: {e}")))?;

        let expected = expected
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_lowercase();

        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let actual = hex::encode(hasher.finalize());

        if actual != expected {
            return Err(crate::Error::ChecksumMismatch);
        }
        debug!("SHA256 verified: {actual}");
    }

    // Write to temp file
    let tmp = tempfile::NamedTempFile::new()
        .map_err(|e| crate::Error::Update(format!("Temp file error: {e}")))?;
    std::fs::write(tmp.path(), &bytes)?;

    let path = tmp
        .into_temp_path()
        .keep()
        .map_err(|e| crate::Error::Update(format!("Temp file persist error: {e}")))?;

    Ok(path)
}

/// Apply the update by replacing the current binary.
/// Returns Ok(()) — a restart is needed to run the new version.
pub fn apply_update(new_binary: &std::path::Path) -> Result<(), crate::Error> {
    let dist = detect_distribution();

    match dist {
        Distribution::Docker => return Err(crate::Error::DockerSelfUpdateNotSupported),
        Distribution::Tauri => {
            return Err(crate::Error::Update(
                "Tauri updates are handled by the Tauri updater plugin".into(),
            ));
        }
        _ => {}
    }

    info!("Applying self-update from {:?}", new_binary);
    self_replace::self_replace(new_binary)
        .map_err(|e| crate::Error::Update(format!("Self-replace failed: {e}")))?;

    info!("Update applied successfully — restart to use new version");
    Ok(())
}

/// Download, verify, and apply an update in one step.
pub async fn download_and_apply(
    client: &reqwest::Client,
    download_url: &str,
    sha256_url: Option<&str>,
) -> Result<(), crate::Error> {
    let asset = ReleaseAsset {
        name: "update".into(),
        browser_download_url: download_url.to_string(),
        size: 0,
    };
    let path = download_and_verify(client, &asset, sha256_url).await?;
    apply_update(&path)?;
    // Clean up temp file
    let _ = std::fs::remove_file(&path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_distribution_default() {
        // Clean env for this test
        std::env::remove_var("AMIGO_DOCKER");
        std::env::remove_var("AMIGO_TAURI");
        let dist = detect_distribution();
        assert_eq!(dist, Distribution::Server);
    }

    #[test]
    fn test_detect_distribution_docker() {
        std::env::set_var("AMIGO_DOCKER", "1");
        std::env::remove_var("AMIGO_TAURI");
        let dist = detect_distribution();
        assert_eq!(dist, Distribution::Docker);
        std::env::remove_var("AMIGO_DOCKER");
    }

    #[test]
    fn test_detect_distribution_tauri() {
        std::env::remove_var("AMIGO_DOCKER");
        std::env::set_var("AMIGO_TAURI", "1");
        let dist = detect_distribution();
        assert_eq!(dist, Distribution::Tauri);
        std::env::remove_var("AMIGO_TAURI");
    }

    #[test]
    fn test_detect_distribution_docker_takes_precedence() {
        // Docker should take precedence over Tauri if both are set
        std::env::set_var("AMIGO_DOCKER", "1");
        std::env::set_var("AMIGO_TAURI", "1");
        let dist = detect_distribution();
        assert_eq!(dist, Distribution::Docker);
        std::env::remove_var("AMIGO_DOCKER");
        std::env::remove_var("AMIGO_TAURI");
    }

    #[test]
    fn test_tauri_cannot_self_update() {
        // When distribution is Tauri, can_self_update should be false
        let dist = Distribution::Tauri;
        let can_self_update = dist == Distribution::Cli || dist == Distribution::Server;
        assert!(!can_self_update);
    }

    #[test]
    fn test_current_version_is_valid_semver() {
        semver::Version::parse(CURRENT_VERSION).expect("CURRENT_VERSION should be valid semver");
    }

    #[test]
    fn test_select_asset() {
        let release = ReleaseInfo {
            tag_name: "v0.2.0".into(),
            name: None,
            body: None,
            assets: vec![
                ReleaseAsset {
                    name: format!(
                        "amigo-server-{}-{}",
                        std::env::consts::OS,
                        std::env::consts::ARCH
                    ),
                    browser_download_url: "https://example.com/binary".into(),
                    size: 1024,
                },
                ReleaseAsset {
                    name: format!(
                        "amigo-server-{}-{}.sha256",
                        std::env::consts::OS,
                        std::env::consts::ARCH
                    ),
                    browser_download_url: "https://example.com/binary.sha256".into(),
                    size: 64,
                },
            ],
        };

        let asset = select_asset(&release);
        assert!(asset.is_some());
        assert!(!asset.unwrap().name.ends_with(".sha256"));
    }
}
