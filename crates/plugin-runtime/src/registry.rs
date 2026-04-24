//! Plugin registry client.
//!
//! Fetches the plugin index from a remote repository (GitHub),
//! compares versions, and downloads plugin files with SHA256 verification.

use std::path::{Path, PathBuf};

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

use crate::types::PluginMeta;

/// Ed25519 public key of the amigo-labs plugin registry signer.
///
/// This is a **placeholder** — replace with the real project key before the
/// 1.0 release. Rotate by shipping a new amigo-server version; clients that
/// haven't updated will reject the new signatures, which is the safer
/// failure mode.
pub const AMIGO_REGISTRY_PUBLIC_KEY: [u8; 32] = [0u8; 32];

/// Registry index as served from the plugin repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryIndex {
    pub schema_version: u32,
    pub plugins: Vec<RegistryPlugin>,
}

/// A plugin entry in the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryPlugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub url_pattern: String,
    pub min_app_version: Option<String>,
    pub sha256: String,
    pub download_url: String,
    pub tags: Vec<String>,
}

/// Describes an available update for a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginUpdateInfo {
    pub plugin_id: String,
    pub current_version: Option<String>,
    pub available_version: String,
    pub is_new: bool,
}

/// Registry configuration.
#[derive(Debug, Clone)]
pub struct RegistryConfig {
    pub index_url: String,
    /// Local cache path for index.json.
    pub cache_path: Option<PathBuf>,
    /// Max age of the cache before auto-refresh (in seconds).
    pub cache_max_age_secs: u64,
    /// Ed25519 public key the remote index must be signed with. `None`
    /// disables signature verification — only use for local development.
    /// Production builds pin this to [`AMIGO_REGISTRY_PUBLIC_KEY`].
    pub trusted_signing_key: Option<[u8; 32]>,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            index_url: "https://raw.githubusercontent.com/amigo-labs/amigo-downloader-plugins/main/index.json".into(),
            cache_path: Some(PathBuf::from("plugins/index.json")),
            cache_max_age_secs: 24 * 60 * 60, // 24 hours
            trusted_signing_key: Some(AMIGO_REGISTRY_PUBLIC_KEY),
        }
    }
}

/// Load the registry index — from local cache if fresh, otherwise fetch remote and cache.
pub async fn load_index(
    client: &reqwest::Client,
    config: &RegistryConfig,
) -> Result<RegistryIndex, crate::Error> {
    // Try local cache first
    if let Some(cache_path) = &config.cache_path
        && let Some(index) = load_cached_index(cache_path, config.cache_max_age_secs) {
            debug!("Using cached registry index from {:?}", cache_path);
            return Ok(index);
        }

    // Fetch from remote
    let index = fetch_index_remote(client, config).await?;

    // Save to cache
    if let Some(cache_path) = &config.cache_path {
        save_cached_index(cache_path, &index);
    }

    Ok(index)
}

/// Force-refresh the registry index from remote, updating the local cache.
pub async fn refresh_index(
    client: &reqwest::Client,
    config: &RegistryConfig,
) -> Result<RegistryIndex, crate::Error> {
    let index = fetch_index_remote(client, config).await?;

    if let Some(cache_path) = &config.cache_path {
        save_cached_index(cache_path, &index);
    }

    Ok(index)
}

/// Load the cached index if it exists and is not expired.
fn load_cached_index(cache_path: &Path, max_age_secs: u64) -> Option<RegistryIndex> {
    let metadata = std::fs::metadata(cache_path).ok()?;
    let modified = metadata.modified().ok()?;
    let age = modified.elapsed().ok()?;

    if age.as_secs() > max_age_secs {
        debug!("Cache expired ({:.0}h old)", age.as_secs() as f64 / 3600.0);
        return None;
    }

    let data = std::fs::read_to_string(cache_path).ok()?;
    let index: RegistryIndex = serde_json::from_str(&data).ok()?;
    Some(index)
}

/// Save the index to the local cache file.
fn save_cached_index(cache_path: &Path, index: &RegistryIndex) {
    if let Some(parent) = cache_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(index) {
        Ok(json) => {
            if let Err(e) = std::fs::write(cache_path, json) {
                debug!("Failed to write index cache: {e}");
            }
        }
        Err(e) => debug!("Failed to serialize index: {e}"),
    }
}

/// Fetch the plugin registry index from the remote URL, verifying its
/// Ed25519 signature when a trusted signing key is configured.
async fn fetch_index_remote(
    client: &reqwest::Client,
    config: &RegistryConfig,
) -> Result<RegistryIndex, crate::Error> {
    debug!("Fetching plugin index: {}", config.index_url);

    let resp = client
        .get(&config.index_url)
        .header("User-Agent", "amigo-downloader")
        .send()
        .await
        .map_err(|e| crate::Error::RegistryUnavailable(e.to_string()))?;

    if !resp.status().is_success() {
        return Err(crate::Error::RegistryUnavailable(format!(
            "Registry returned HTTP {}",
            resp.status()
        )));
    }

    // We need the raw bytes to verify the signature over the *exact*
    // wire-format, not a re-serialised copy.
    let raw = resp
        .bytes()
        .await
        .map_err(|e| crate::Error::RegistryUnavailable(e.to_string()))?;

    match &config.trusted_signing_key {
        Some(pubkey) => {
            let sig_url = format!("{}.sig", config.index_url);
            let sig_hex = fetch_signature(client, &sig_url).await?;
            verify_ed25519(&raw, &sig_hex, pubkey)?;
            debug!("Registry index signature verified");
        }
        None => warn!(
            "Registry signature verification DISABLED — only safe for local development"
        ),
    }

    let index: RegistryIndex = serde_json::from_slice(&raw)
        .map_err(|e| crate::Error::RegistryUnavailable(format!("Invalid index: {e}")))?;

    info!("Registry index: {} plugins available", index.plugins.len());
    Ok(index)
}

/// Fetch the detached signature file alongside `index.json` (same URL with
/// a `.sig` suffix). The body is expected to be a hex-encoded Ed25519
/// signature — 64 bytes / 128 hex chars, optionally trailing whitespace.
async fn fetch_signature(
    client: &reqwest::Client,
    sig_url: &str,
) -> Result<String, crate::Error> {
    let resp = client
        .get(sig_url)
        .header("User-Agent", "amigo-downloader")
        .send()
        .await
        .map_err(|e| {
            crate::Error::RegistryUnavailable(format!(
                "registry signature fetch failed ({sig_url}): {e}"
            ))
        })?;
    if !resp.status().is_success() {
        return Err(crate::Error::RegistryUnavailable(format!(
            "registry signature missing — HTTP {} on {sig_url}",
            resp.status()
        )));
    }
    resp.text().await.map_err(|e| {
        crate::Error::RegistryUnavailable(format!("signature body unreadable: {e}"))
    })
}

/// Verify that `payload` carries the Ed25519 signature `sig_hex` produced
/// by the private key matching `pubkey`.
pub fn verify_ed25519(
    payload: &[u8],
    sig_hex: &str,
    pubkey: &[u8; 32],
) -> Result<(), crate::Error> {
    let sig_bytes = hex::decode(sig_hex.trim()).map_err(|e| {
        crate::Error::RegistryUnavailable(format!("signature is not hex: {e}"))
    })?;
    if sig_bytes.len() != 64 {
        return Err(crate::Error::RegistryUnavailable(format!(
            "signature has wrong length {} (want 64)",
            sig_bytes.len()
        )));
    }
    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig_bytes);
    let sig = Signature::from_bytes(&sig_arr);
    let vk = VerifyingKey::from_bytes(pubkey).map_err(|e| {
        crate::Error::RegistryUnavailable(format!("bad registry pubkey: {e}"))
    })?;
    vk.verify(payload, &sig).map_err(|_| {
        crate::Error::RegistryUnavailable(
            "registry signature did not verify against the trusted key".into(),
        )
    })
}

/// Legacy alias — use `load_index` instead.
pub async fn fetch_index(
    client: &reqwest::Client,
    config: &RegistryConfig,
) -> Result<RegistryIndex, crate::Error> {
    load_index(client, config).await
}

/// Compare installed plugins against registry to find available updates.
pub fn check_plugin_updates(
    index: &RegistryIndex,
    installed: &[PluginMeta],
) -> Vec<PluginUpdateInfo> {
    let mut updates = Vec::new();

    for registry_plugin in &index.plugins {
        let local = installed.iter().find(|p| p.id == registry_plugin.id);

        match local {
            Some(local_plugin) => {
                // Compare versions
                let local_ver = semver::Version::parse(&local_plugin.version).ok();
                let remote_ver = semver::Version::parse(&registry_plugin.version).ok();

                if let (Some(local_v), Some(remote_v)) = (local_ver, remote_ver)
                    && remote_v > local_v
                {
                    updates.push(PluginUpdateInfo {
                        plugin_id: registry_plugin.id.clone(),
                        current_version: Some(local_plugin.version.clone()),
                        available_version: registry_plugin.version.clone(),
                        is_new: false,
                    });
                }
            }
            None => {
                // New plugin not installed locally
                updates.push(PluginUpdateInfo {
                    plugin_id: registry_plugin.id.clone(),
                    current_version: None,
                    available_version: registry_plugin.version.clone(),
                    is_new: true,
                });
            }
        }
    }

    updates
}

/// Find a registry plugin whose url_pattern matches the given URL.
pub fn suggest_plugin_for_url<'a>(
    index: &'a RegistryIndex,
    url: &str,
) -> Option<&'a RegistryPlugin> {
    for plugin in &index.plugins {
        if let Ok(re) = regex::Regex::new(&plugin.url_pattern)
            && re.is_match(url) {
                return Some(plugin);
            }
    }
    None
}

/// Download a plugin file, verify SHA256, and install into a plugin folder.
/// Creates `<dest_dir>/<plugin-id>/plugin.ts` (or .js based on download URL).
pub async fn download_plugin(
    client: &reqwest::Client,
    registry_plugin: &RegistryPlugin,
    dest_dir: &Path,
) -> Result<PathBuf, crate::Error> {
    info!(
        "Downloading plugin {} v{} from {}",
        registry_plugin.id, registry_plugin.version, registry_plugin.download_url
    );

    let resp = client
        .get(&registry_plugin.download_url)
        .send()
        .await
        .map_err(|e| crate::Error::RegistryUnavailable(e.to_string()))?
        .error_for_status()
        .map_err(|e| crate::Error::RegistryUnavailable(e.to_string()))?;

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| crate::Error::RegistryUnavailable(e.to_string()))?;

    // Verify SHA256
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual_hash = hex::encode(hasher.finalize());

    if actual_hash != registry_plugin.sha256.to_lowercase() {
        return Err(crate::Error::ChecksumMismatch(registry_plugin.id.clone()));
    }
    debug!("SHA256 verified for plugin {}", registry_plugin.id);

    // Determine extension from download URL
    let ext = if registry_plugin.download_url.ends_with(".ts") {
        "ts"
    } else {
        "js"
    };

    // Install into <dest_dir>/<plugin-id>/plugin.ts
    let plugin_dir = dest_dir.join(&registry_plugin.id);
    std::fs::create_dir_all(&plugin_dir)
        .map_err(|e| crate::Error::Other(format!("Failed to create dir: {e}")))?;

    let final_path = plugin_dir.join(format!("plugin.{ext}"));
    let tmp_path = plugin_dir.join(format!("plugin.{ext}.new"));

    std::fs::write(&tmp_path, &bytes)
        .map_err(|e| crate::Error::Other(format!("Failed to write plugin: {e}")))?;

    std::fs::rename(&tmp_path, &final_path)
        .map_err(|e| crate::Error::Other(format!("Failed to rename plugin: {e}")))?;

    info!(
        "Plugin {} installed at {:?}",
        registry_plugin.id, final_path
    );
    Ok(final_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_plugin_updates_detects_update() {
        let index = RegistryIndex {
            schema_version: 1,
            plugins: vec![RegistryPlugin {
                id: "test-plugin".into(),
                name: "Test".into(),
                version: "2.0.0".into(),
                description: "Test plugin".into(),
                author: "test".into(),
                url_pattern: ".*".into(),
                min_app_version: None,
                sha256: "abc".into(),
                download_url: "https://example.com/test.rn".into(),
                tags: vec![],
            }],
        };

        let installed = vec![PluginMeta {
            id: "test-plugin".into(),
            name: "Test".into(),
            version: "1.0.0".into(),
            url_pattern: ".*".into(),
            file_path: "/tmp/test.rn".into(),
            enabled: true,
            description: None,
            author: None,
            plugin_type: crate::types::PluginType::default(),
        }];

        let updates = check_plugin_updates(&index, &installed);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].plugin_id, "test-plugin");
        assert_eq!(updates[0].available_version, "2.0.0");
        assert!(!updates[0].is_new);
    }

    #[test]
    fn test_check_plugin_updates_no_update_when_current() {
        let index = RegistryIndex {
            schema_version: 1,
            plugins: vec![RegistryPlugin {
                id: "test-plugin".into(),
                name: "Test".into(),
                version: "1.0.0".into(),
                description: "Test".into(),
                author: "test".into(),
                url_pattern: ".*".into(),
                min_app_version: None,
                sha256: "abc".into(),
                download_url: "https://example.com/test.rn".into(),
                tags: vec![],
            }],
        };

        let installed = vec![PluginMeta {
            id: "test-plugin".into(),
            name: "Test".into(),
            version: "1.0.0".into(),
            url_pattern: ".*".into(),
            file_path: "/tmp/test.rn".into(),
            enabled: true,
            description: None,
            author: None,
            plugin_type: crate::types::PluginType::default(),
        }];

        let updates = check_plugin_updates(&index, &installed);
        assert!(updates.is_empty());
    }

    #[test]
    fn test_check_plugin_updates_detects_new_plugin() {
        let index = RegistryIndex {
            schema_version: 1,
            plugins: vec![RegistryPlugin {
                id: "new-plugin".into(),
                name: "New".into(),
                version: "1.0.0".into(),
                description: "New plugin".into(),
                author: "test".into(),
                url_pattern: ".*".into(),
                min_app_version: None,
                sha256: "abc".into(),
                download_url: "https://example.com/new.rn".into(),
                tags: vec![],
            }],
        };

        let installed: Vec<PluginMeta> = vec![];
        let updates = check_plugin_updates(&index, &installed);
        assert_eq!(updates.len(), 1);
        assert!(updates[0].is_new);
    }

    #[test]
    fn verify_ed25519_accepts_valid_signature() {
        use ed25519_dalek::{Signer, SigningKey};
        let signer = SigningKey::from_bytes(&[7u8; 32]);
        let pubkey = signer.verifying_key().to_bytes();
        let payload = br#"{"schema_version":1,"plugins":[]}"#;
        let sig = signer.sign(payload);
        let sig_hex = hex::encode(sig.to_bytes());

        verify_ed25519(payload, &sig_hex, &pubkey).expect("valid sig should verify");
    }

    #[test]
    fn verify_ed25519_rejects_tampered_payload() {
        use ed25519_dalek::{Signer, SigningKey};
        let signer = SigningKey::from_bytes(&[9u8; 32]);
        let pubkey = signer.verifying_key().to_bytes();
        let sig = signer.sign(b"original");
        let sig_hex = hex::encode(sig.to_bytes());

        let err = verify_ed25519(b"tampered", &sig_hex, &pubkey)
            .expect_err("tampered payload must not verify");
        assert!(format!("{err}").contains("signature did not verify"));
    }

    #[test]
    fn verify_ed25519_rejects_wrong_key() {
        use ed25519_dalek::{Signer, SigningKey};
        let signer = SigningKey::from_bytes(&[1u8; 32]);
        let other = SigningKey::from_bytes(&[2u8; 32]);
        let payload = b"payload";
        let sig = signer.sign(payload);
        let sig_hex = hex::encode(sig.to_bytes());

        let err = verify_ed25519(payload, &sig_hex, &other.verifying_key().to_bytes())
            .expect_err("signature signed by a different key must not verify");
        assert!(format!("{err}").contains("signature did not verify"));
    }

    #[test]
    fn verify_ed25519_rejects_malformed_hex() {
        let pk = [0u8; 32];
        let err = verify_ed25519(b"x", "not-hex", &pk).expect_err("malformed hex must error");
        assert!(format!("{err}").contains("not hex"));
    }

    #[test]
    fn verify_ed25519_rejects_short_signature() {
        let pk = [0u8; 32];
        let err =
            verify_ed25519(b"x", "aa", &pk).expect_err("short signature must error");
        assert!(format!("{err}").contains("wrong length"));
    }

    #[test]
    fn test_deserialize_registry_index() {
        let json = r#"{
            "schema_version": 1,
            "plugins": [{
                "id": "mega-nz",
                "name": "MEGA.nz",
                "version": "1.0.0",
                "description": "MEGA download support",
                "author": "amigo-labs",
                "url_pattern": "https?://mega\\.nz/.+",
                "sha256": "abcdef",
                "download_url": "https://example.com/mega.rn",
                "tags": ["filehost"]
            }]
        }"#;

        let index: RegistryIndex = serde_json::from_str(json).unwrap();
        assert_eq!(index.schema_version, 1);
        assert_eq!(index.plugins.len(), 1);
        assert_eq!(index.plugins[0].id, "mega-nz");
    }
}
