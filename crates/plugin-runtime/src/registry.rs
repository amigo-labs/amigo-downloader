//! Plugin registry client.
//!
//! Fetches the plugin index from a remote repository (GitHub),
//! compares versions, and downloads plugin files with SHA256 verification.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use ed25519_dalek::{Signature, VerifyingKey};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

use crate::types::PluginMeta;

/// Cache of compiled URL-pattern regexes, keyed by the pattern string. The
/// suggest endpoint recompiled every plugin's pattern on every request; the
/// patterns are stable, so compile each at most once across requests.
static URL_PATTERN_CACHE: LazyLock<Mutex<HashMap<String, Option<Regex>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Get-or-compile the regex for a plugin URL pattern. Invalid patterns cache a
/// `None` sentinel so they're skipped (as before) without retrying compilation.
fn url_pattern_regex(pattern: &str) -> Option<Regex> {
    let mut cache = URL_PATTERN_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache
        .entry(pattern.to_string())
        .or_insert_with(|| Regex::new(pattern).ok())
        .clone()
}

/// Ed25519 public key of the amigo-labs plugin registry signer.
///
/// Injected at compile time from the `AMIGO_REGISTRY_PUBKEY_HEX` environment
/// variable (64 hex chars). When unset the value falls back to the all-zero
/// placeholder, which no [`RegistryConfig`] constructor accepts: signature
/// verification is disabled (with a runtime warning) in dev builds instead of
/// trusting a forgeable key. `build.rs` refuses to compile a release build
/// when the env var is unset, so production binaries always pin a real key.
///
/// Key rotation is intentionally fail-closed: rotate by shipping a new
/// amigo-server release; clients that haven't updated reject the new
/// signatures and silently lose marketplace access until they update. A
/// second, overlapping trusted key would make rotation non-breaking — that is
/// a deliberate follow-up, not an oversight.
pub const AMIGO_REGISTRY_PUBLIC_KEY: [u8; 32] = registry_pubkey_from_env();

const ZERO_PUBKEY: [u8; 32] = [0u8; 32];

const fn registry_pubkey_from_env() -> [u8; 32] {
    match option_env!("AMIGO_REGISTRY_PUBKEY_HEX") {
        Some(hex) => parse_hex32(hex),
        None => ZERO_PUBKEY,
    }
}

const fn parse_hex32(s: &str) -> [u8; 32] {
    let bytes = s.as_bytes();
    assert!(
        bytes.len() == 64,
        "AMIGO_REGISTRY_PUBKEY_HEX must be 64 hex chars (32-byte Ed25519 public key)"
    );
    let mut out = [0u8; 32];
    let mut i = 0;
    while i < 32 {
        out[i] = (hex_nibble(bytes[i * 2]) << 4) | hex_nibble(bytes[i * 2 + 1]);
        i += 1;
    }
    out
}

const fn hex_nibble(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => panic!("AMIGO_REGISTRY_PUBKEY_HEX contains non-hex character"),
    }
}

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
    /// Host-API major the plugin was written against (see
    /// [`crate::HOST_API_VERSION`]). Missing means 1, the version that predates
    /// the field.
    #[serde(default)]
    pub api_version: Option<u32>,
    /// Permissions the plugin declares — currently the domains it may reach.
    /// Shown before install and compared on update so a widened set is never
    /// applied silently.
    #[serde(default)]
    pub permissions: Option<crate::types::PluginPermissions>,
}

impl RegistryPlugin {
    /// Host-API major this entry requires (missing = 1).
    pub fn required_api_version(&self) -> u32 {
        self.api_version.unwrap_or(1)
    }

    /// Whether the running host can load this plugin.
    pub fn is_compatible(&self) -> bool {
        crate::is_supported_api_version(self.required_api_version())
    }

    /// Domains the plugin declares, `None` when it declares no allowlist.
    pub fn declared_domains(&self) -> Option<&[String]> {
        self.permissions.as_ref().and_then(|p| p.domains.as_deref())
    }
}

/// Cap on the registry index body. The real index is a few KiB per plugin.
const MAX_INDEX_BYTES: usize = 4 * 1024 * 1024;

/// Cap on the detached signature body (128 hex chars plus whitespace).
const MAX_SIGNATURE_BYTES: usize = 1024;

/// Cap on a single plugin artifact. Plugins are one TypeScript/JavaScript
/// source file; 2 MiB is far beyond any legitimate one.
pub const MAX_PLUGIN_ARTIFACT_BYTES: usize = 2 * 1024 * 1024;

/// Validate a plugin id before it is used as a filesystem path component.
/// Accepts `^[a-z0-9][a-z0-9-]{0,63}$` — no separators, no dots, no traversal.
pub fn validate_plugin_id(id: &str) -> Result<(), crate::Error> {
    let bytes = id.as_bytes();
    let valid = !bytes.is_empty()
        && bytes.len() <= 64
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-');
    if valid {
        Ok(())
    } else {
        Err(crate::Error::SandboxViolation(format!(
            "invalid plugin id {id:?}: must match ^[a-z0-9][a-z0-9-]{{0,63}}$"
        )))
    }
}

/// Describes an available update for a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginUpdateInfo {
    pub plugin_id: String,
    pub current_version: Option<String>,
    pub available_version: String,
    pub is_new: bool,
    /// Domains the new version declares, `None` when it declares none
    /// (unscoped).
    #[serde(default)]
    pub declared_domains: Option<Vec<String>>,
    /// Domains the update adds to what the installed version may reach
    /// (`["*"]` when the update drops its allowlist). `None` for a new plugin
    /// or an update that does not widen access.
    #[serde(default)]
    pub added_domains: Option<Vec<String>>,
    /// The update must be confirmed by the user before it is applied — it is
    /// never installed automatically.
    #[serde(default)]
    pub requires_approval: bool,
}

/// Registry configuration.
#[derive(Debug, Clone)]
pub struct RegistryConfig {
    pub index_url: String,
    /// Local cache path for index.json. The exact signed bytes are stored
    /// here, with the detached signature next to it (`<cache_path>.sig`).
    pub cache_path: Option<PathBuf>,
    /// Max age of the cache before auto-refresh (in seconds).
    pub cache_max_age_secs: u64,
    /// Ed25519 public key the index must be signed with. `None` disables
    /// signature verification — only for local development. Private so the
    /// only ways to set it are the constructors below, none of which accept
    /// the all-zero placeholder or another low-order key.
    index_verifier: Option<[u8; 32]>,
}

/// The compile-time key, unless it is the all-zero placeholder.
fn compiled_in_verifier() -> Option<[u8; 32]> {
    // If no real key was injected at compile time, refuse to trust the zero
    // placeholder. Disabling verification with a warning is safer than
    // verifying against a key whose private half is publicly derivable.
    if AMIGO_REGISTRY_PUBLIC_KEY == ZERO_PUBKEY {
        None
    } else {
        Some(AMIGO_REGISTRY_PUBLIC_KEY)
    }
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            index_url: "https://raw.githubusercontent.com/amigo-labs/amigo-downloader-plugins/main/index.json".into(),
            cache_path: Some(PathBuf::from("plugins/index.json")),
            cache_max_age_secs: 24 * 60 * 60, // 24 hours
            index_verifier: compiled_in_verifier(),
        }
    }
}

impl RegistryConfig {
    /// The configuration the server uses: the compiled-in key (via
    /// [`Default`]), a custom index URL, and an explicit opt-out for working
    /// against an unsigned local fork.
    pub fn for_index(index_url: impl Into<String>, dev_unsigned: bool) -> Self {
        let config = Self {
            index_url: index_url.into(),
            ..Default::default()
        };
        if dev_unsigned {
            config.without_signature_verification()
        } else {
            config
        }
    }

    /// Disable signature verification. Development only.
    pub fn without_signature_verification(mut self) -> Self {
        self.index_verifier = None;
        self
    }

    /// Pin the Ed25519 public key that index signatures are verified
    /// against. Rejects keys that are not valid, or are low-order (weak) —
    /// which includes the all-zero placeholder.
    pub fn with_index_verifier(mut self, verifier: [u8; 32]) -> Result<Self, crate::Error> {
        let vk = VerifyingKey::from_bytes(&verifier)
            .map_err(|e| crate::Error::RegistryUnavailable(format!("bad registry pubkey: {e}")))?;
        if vk.is_weak() {
            return Err(crate::Error::RegistryUnavailable(
                "refusing a low-order registry signing key".into(),
            ));
        }
        self.index_verifier = Some(verifier);
        Ok(self)
    }

    /// The key index signatures are verified against, if any.
    pub fn index_verifier(&self) -> Option<&[u8; 32]> {
        self.index_verifier.as_ref()
    }
}

/// Load the registry index — from local cache if fresh, otherwise fetch remote and cache.
pub async fn load_index(
    client: &reqwest::Client,
    config: &RegistryConfig,
) -> Result<RegistryIndex, crate::Error> {
    // Try local cache first. A cache that fails signature verification is a
    // miss, not an error: refetch and overwrite it.
    if let Some(cache_path) = &config.cache_path
        && let Some(index) = load_cached_index(cache_path, config)
    {
        debug!("Using cached registry index from {:?}", cache_path);
        return Ok(index);
    }

    refresh_index(client, config).await
}

/// Force-refresh the registry index from remote, updating the local cache.
pub async fn refresh_index(
    client: &reqwest::Client,
    config: &RegistryConfig,
) -> Result<RegistryIndex, crate::Error> {
    let fetched = fetch_index_remote(client, config).await?;

    if let Some(cache_path) = &config.cache_path {
        save_cached_index(cache_path, &fetched);
    }

    Ok(fetched.index)
}

/// Path of the detached signature stored next to a cached index.
fn cached_signature_path(cache_path: &Path) -> PathBuf {
    let mut p = cache_path.as_os_str().to_owned();
    p.push(".sig");
    PathBuf::from(p)
}

/// Load the cached index if it exists, is not expired and — when a signing
/// key is configured — still carries a valid signature over its exact bytes.
fn load_cached_index(cache_path: &Path, config: &RegistryConfig) -> Option<RegistryIndex> {
    let metadata = std::fs::metadata(cache_path).ok()?;
    let modified = metadata.modified().ok()?;
    let age = modified.elapsed().ok()?;

    if age.as_secs() > config.cache_max_age_secs {
        debug!("Cache expired ({:.0}h old)", age.as_secs() as f64 / 3600.0);
        return None;
    }
    if metadata.len() > MAX_INDEX_BYTES as u64 {
        warn!("Cached registry index is oversized — refetching");
        return None;
    }

    let raw = std::fs::read(cache_path).ok()?;
    if let Some(pubkey) = config.index_verifier() {
        let Ok(sig_hex) = std::fs::read_to_string(cached_signature_path(cache_path)) else {
            warn!("Cached registry index has no signature — refetching");
            return None;
        };
        if let Err(e) = verify_ed25519(&raw, &sig_hex, pubkey) {
            warn!("Cached registry index failed verification ({e}) — refetching");
            return None;
        }
    }
    serde_json::from_slice(&raw).ok()
}

/// Save the exact fetched bytes (and the signature, when there is one) to the
/// local cache, so the next read can re-verify them.
fn save_cached_index(cache_path: &Path, fetched: &FetchedIndex) {
    if let Some(parent) = cache_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let sig_path = cached_signature_path(cache_path);
    // Write the signature first: a crash between the two writes then leaves a
    // signature that doesn't match the old index, which reads as a miss.
    let sig_result = match &fetched.signature_hex {
        Some(sig) => std::fs::write(&sig_path, sig),
        None => match std::fs::remove_file(&sig_path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
    };
    if let Err(e) = sig_result {
        debug!("Failed to write index signature cache: {e}");
        return;
    }
    if let Err(e) = std::fs::write(cache_path, &fetched.raw) {
        debug!("Failed to write index cache: {e}");
    }
}

/// A freshly fetched index together with the exact bytes it was parsed from.
struct FetchedIndex {
    index: RegistryIndex,
    raw: Vec<u8>,
    signature_hex: Option<String>,
}

/// Fetch the plugin registry index from the remote URL, verifying its
/// Ed25519 signature when a trusted signing key is configured.
async fn fetch_index_remote(
    client: &reqwest::Client,
    config: &RegistryConfig,
) -> Result<FetchedIndex, crate::Error> {
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
    let raw = read_capped(resp, MAX_INDEX_BYTES, "registry index").await?;

    let signature_hex = match config.index_verifier() {
        Some(pubkey) => {
            let sig_url = format!("{}.sig", config.index_url);
            let sig_hex = fetch_signature(client, &sig_url).await?;
            verify_ed25519(&raw, &sig_hex, pubkey)?;
            debug!("Registry index signature verified");
            Some(sig_hex)
        }
        None => {
            warn!("Registry signature verification DISABLED — only safe for local development");
            None
        }
    };

    let index: RegistryIndex = serde_json::from_slice(&raw)
        .map_err(|e| crate::Error::RegistryUnavailable(format!("Invalid index: {e}")))?;

    info!("Registry index: {} plugins available", index.plugins.len());
    Ok(FetchedIndex {
        index,
        raw,
        signature_hex,
    })
}

/// Buffer a response body, failing once it exceeds `max` bytes. Checks the
/// declared `Content-Length` up front and the actual bytes as they stream, so
/// a missing or lying header cannot bypass the cap.
async fn read_capped(
    mut resp: reqwest::Response,
    max: usize,
    what: &str,
) -> Result<Vec<u8>, crate::Error> {
    if let Some(len) = resp.content_length()
        && len > max as u64
    {
        return Err(crate::Error::RegistryUnavailable(format!(
            "{what} too large ({len} bytes; limit {max})"
        )));
    }
    let mut buf = Vec::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| crate::Error::RegistryUnavailable(e.to_string()))?
    {
        if buf.len() + chunk.len() > max {
            return Err(crate::Error::RegistryUnavailable(format!(
                "{what} exceeded limit of {max} bytes"
            )));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

/// Fetch the detached signature file alongside `index.json` (same URL with
/// a `.sig` suffix). The body is expected to be a hex-encoded Ed25519
/// signature — 64 bytes / 128 hex chars, optionally trailing whitespace.
async fn fetch_signature(client: &reqwest::Client, sig_url: &str) -> Result<String, crate::Error> {
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
    let raw = read_capped(resp, MAX_SIGNATURE_BYTES, "registry signature").await?;
    String::from_utf8(raw)
        .map_err(|e| crate::Error::RegistryUnavailable(format!("signature body unreadable: {e}")))
}

/// Verify that `payload` carries the Ed25519 signature `sig_hex` produced
/// by the private key matching `pubkey`. Uses `verify_strict`, which rejects
/// low-order public keys (such as the all-zero placeholder) and
/// non-canonical signatures regardless of how the key got here.
pub fn verify_ed25519(
    payload: &[u8],
    sig_hex: &str,
    pubkey: &[u8; 32],
) -> Result<(), crate::Error> {
    let sig_bytes = hex::decode(sig_hex.trim())
        .map_err(|e| crate::Error::RegistryUnavailable(format!("signature is not hex: {e}")))?;
    if sig_bytes.len() != 64 {
        return Err(crate::Error::RegistryUnavailable(format!(
            "signature has wrong length {} (want 64)",
            sig_bytes.len()
        )));
    }
    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig_bytes);
    let sig = Signature::from_bytes(&sig_arr);
    let vk = VerifyingKey::from_bytes(pubkey)
        .map_err(|e| crate::Error::RegistryUnavailable(format!("bad registry pubkey: {e}")))?;
    vk.verify_strict(payload, &sig).map_err(|_| {
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
/// Entries the running host cannot load (unsupported `api_version`) are
/// skipped — they are neither offered nor installed.
pub fn check_plugin_updates(
    index: &RegistryIndex,
    installed: &[PluginMeta],
) -> Vec<PluginUpdateInfo> {
    let mut updates = Vec::new();

    for registry_plugin in &index.plugins {
        if !registry_plugin.is_compatible() {
            debug!(
                "Skipping {} v{}: needs host API {}",
                registry_plugin.id,
                registry_plugin.version,
                registry_plugin.required_api_version()
            );
            continue;
        }
        let local = installed.iter().find(|p| p.id == registry_plugin.id);
        let declared_domains = registry_plugin.declared_domains().map(<[String]>::to_vec);

        match local {
            Some(local_plugin) => {
                // Compare versions
                let local_ver = semver::Version::parse(&local_plugin.version).ok();
                let remote_ver = semver::Version::parse(&registry_plugin.version).ok();

                if let (Some(local_v), Some(remote_v)) = (local_ver, remote_ver)
                    && remote_v > local_v
                {
                    let added_domains = crate::permissions::widened_domains(
                        local_plugin.permissions.domains.as_deref(),
                        registry_plugin.declared_domains(),
                    );
                    updates.push(PluginUpdateInfo {
                        plugin_id: registry_plugin.id.clone(),
                        current_version: Some(local_plugin.version.clone()),
                        available_version: registry_plugin.version.clone(),
                        is_new: false,
                        declared_domains,
                        requires_approval: added_domains.is_some(),
                        added_domains,
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
                    declared_domains,
                    added_domains: None,
                    // Installing is always an explicit, confirmed action.
                    requires_approval: true,
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
        if let Some(re) = url_pattern_regex(&plugin.url_pattern)
            && re.is_match(url)
        {
            return Some(plugin);
        }
    }
    None
}

/// File extension for a plugin source artifact, from its download URL's path.
/// Anything other than `.ts` / `.js` (bytecode, archives, binaries) is
/// rejected.
fn source_extension(download_url: &str) -> Result<&'static str, crate::Error> {
    let path = url::Url::parse(download_url)
        .map(|u| u.path().to_ascii_lowercase())
        .map_err(|e| crate::Error::RegistryUnavailable(format!("invalid download_url: {e}")))?;
    if path.ends_with(".ts") {
        Ok("ts")
    } else if path.ends_with(".js") {
        Ok("js")
    } else {
        Err(crate::Error::SandboxViolation(format!(
            "registry artifact {download_url} is not a .ts/.js source file"
        )))
    }
}

/// Download a plugin file, verify SHA256, and install into a plugin folder.
/// Creates `<dest_dir>/<plugin-id>/plugin.ts` (or .js based on download URL).
pub async fn download_plugin(
    client: &reqwest::Client,
    registry_plugin: &RegistryPlugin,
    dest_dir: &Path,
) -> Result<PathBuf, crate::Error> {
    let artifact = fetch_plugin_artifact(client, registry_plugin).await?;
    install_artifact(&dest_dir.join(&registry_plugin.id), &artifact)
}

/// A downloaded, checksum-verified plugin source artifact that has not been
/// written into the plugin directory yet.
#[derive(Debug, Clone)]
pub struct PluginArtifact {
    /// Registry id the artifact was fetched for (already validated).
    pub id: String,
    /// `"ts"` or `"js"`.
    pub ext: &'static str,
    /// UTF-8 source bytes.
    pub bytes: Vec<u8>,
}

/// Download a plugin artifact and verify it against its signed registry
/// entry (id format, source-only, host-API compatibility, size cap, SHA-256,
/// UTF-8) without touching the plugin directory.
pub async fn fetch_plugin_artifact(
    client: &reqwest::Client,
    registry_plugin: &RegistryPlugin,
) -> Result<PluginArtifact, crate::Error> {
    // The id becomes a directory name — validate before touching the disk.
    validate_plugin_id(&registry_plugin.id)?;
    // The registry serves plugin *source* only. Compiled QuickJS bytecode is
    // never accepted from a remote (it is not validated before execution);
    // it is only ever produced locally, see `crate::bytecode_cache`.
    let ext = source_extension(&registry_plugin.download_url)?;
    if !registry_plugin.is_compatible() {
        return Err(crate::Error::IncompatibleVersion {
            required: format!("host API {}", registry_plugin.required_api_version()),
            current: format!("host API {}", crate::HOST_API_VERSION),
        });
    }

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

    let bytes = read_capped(resp, MAX_PLUGIN_ARTIFACT_BYTES, "plugin artifact").await?;

    // Verify SHA256
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual_hash = hex::encode(hasher.finalize());

    if actual_hash != registry_plugin.sha256.to_lowercase() {
        return Err(crate::Error::ChecksumMismatch(registry_plugin.id.clone()));
    }
    debug!("SHA256 verified for plugin {}", registry_plugin.id);

    if std::str::from_utf8(&bytes).is_err() {
        return Err(crate::Error::SandboxViolation(format!(
            "plugin {} artifact is not UTF-8 source text",
            registry_plugin.id
        )));
    }

    Ok(PluginArtifact {
        id: registry_plugin.id.clone(),
        ext,
        bytes,
    })
}

/// Write `artifact` as `<plugin_dir>/plugin.<ext>` (atomic write-then-rename)
/// and remove a stale entry file with the other extension, so the loader
/// cannot pick up the previous version next to the new one.
pub fn install_artifact(
    plugin_dir: &Path,
    artifact: &PluginArtifact,
) -> Result<PathBuf, crate::Error> {
    std::fs::create_dir_all(plugin_dir)
        .map_err(|e| crate::Error::Other(format!("Failed to create dir: {e}")))?;

    let final_path = plugin_dir.join(format!("plugin.{}", artifact.ext));
    let tmp_path = plugin_dir.join(format!("plugin.{}.new", artifact.ext));

    std::fs::write(&tmp_path, &artifact.bytes)
        .map_err(|e| crate::Error::Other(format!("Failed to write plugin: {e}")))?;

    std::fs::rename(&tmp_path, &final_path)
        .map_err(|e| crate::Error::Other(format!("Failed to rename plugin: {e}")))?;

    let other = if artifact.ext == "ts" { "js" } else { "ts" };
    let stale = plugin_dir.join(format!("plugin.{other}"));
    if stale.exists() {
        std::fs::remove_file(&stale)
            .map_err(|e| crate::Error::Other(format!("Failed to remove stale plugin: {e}")))?;
    }

    info!("Plugin {} installed at {:?}", artifact.id, final_path);
    Ok(final_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin_with_pattern(id: &str, pattern: &str) -> RegistryPlugin {
        RegistryPlugin {
            id: id.into(),
            name: id.into(),
            version: "1.0.0".into(),
            description: String::new(),
            author: String::new(),
            url_pattern: pattern.into(),
            min_app_version: None,
            sha256: String::new(),
            download_url: "https://example.com/p.ts".into(),
            tags: vec![],
            api_version: None,
            permissions: None,
        }
    }

    #[test]
    fn test_suggest_plugin_for_url_matches_and_is_stable() {
        let index = RegistryIndex {
            schema_version: 1,
            plugins: vec![
                plugin_with_pattern("mega", r"https?://mega\.nz/.+"),
                plugin_with_pattern("example", r"https?://example\.com/.+"),
            ],
        };

        // Same query twice exercises the compiled-pattern cache path.
        let url = "https://example.com/file/123";
        assert_eq!(suggest_plugin_for_url(&index, url).unwrap().id, "example");
        assert_eq!(suggest_plugin_for_url(&index, url).unwrap().id, "example");
        assert!(suggest_plugin_for_url(&index, "https://nomatch.test/x").is_none());
    }

    #[test]
    fn test_url_pattern_regex_caches_invalid_as_none() {
        // An invalid pattern must yield None (skipped) without panicking,
        // and a repeated lookup must stay None.
        assert!(url_pattern_regex("(((unbalanced").is_none());
        assert!(url_pattern_regex("(((unbalanced").is_none());
    }

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
                api_version: None,
                permissions: None,
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
            api_version: 1,
            permissions: Default::default(),
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
                api_version: None,
                permissions: None,
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
            api_version: 1,
            permissions: Default::default(),
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
                api_version: None,
                permissions: None,
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
        let err = verify_ed25519(b"x", "aa", &pk).expect_err("short signature must error");
        assert!(format!("{err}").contains("wrong length"));
    }

    #[test]
    fn default_config_disables_verification_when_pubkey_is_zero() {
        // The default impl must never trust the all-zero placeholder pubkey.
        // When no real key was injected at compile time the field has to be
        // None so that fetch_index_remote logs the explicit "verification
        // DISABLED" warning instead of silently accepting a forgeable signature.
        if AMIGO_REGISTRY_PUBLIC_KEY == ZERO_PUBKEY {
            let cfg = RegistryConfig::default();
            assert!(
                cfg.index_verifier.is_none(),
                "default must not trust the zero placeholder"
            );
        } else {
            let cfg = RegistryConfig::default();
            assert_eq!(cfg.index_verifier, Some(AMIGO_REGISTRY_PUBLIC_KEY));
        }
    }

    #[test]
    fn parse_hex32_round_trip() {
        let pk = parse_hex32("0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20");
        assert_eq!(pk[0], 0x01);
        assert_eq!(pk[15], 0x10);
        assert_eq!(pk[31], 0x20);
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

    // --- #80: signature-verification bypasses ---------------------------

    #[test]
    fn server_construction_path_never_trusts_the_zero_key() {
        // `for_index` is what amigo-server uses. In a build without a real
        // key it must disable verification instead of pinning [0; 32].
        let cfg = RegistryConfig::for_index("https://example.com/index.json", false);
        assert_ne!(cfg.index_verifier(), Some(&ZERO_PUBKEY));
        assert_eq!(cfg.index_verifier().copied(), compiled_in_verifier());
        let dev = RegistryConfig::for_index("https://example.com/index.json", true);
        assert!(dev.index_verifier().is_none());
    }

    #[test]
    fn explicit_low_order_keys_are_rejected() {
        assert!(
            RegistryConfig::default()
                .with_index_verifier(ZERO_PUBKEY)
                .is_err()
        );
        // The identity point (y = 1) is another small-order key.
        let mut identity = [0u8; 32];
        identity[0] = 1;
        assert!(
            RegistryConfig::default()
                .with_index_verifier(identity)
                .is_err()
        );
        use ed25519_dalek::SigningKey;
        let real = SigningKey::from_bytes(&[3u8; 32])
            .verifying_key()
            .to_bytes();
        assert!(RegistryConfig::default().with_index_verifier(real).is_ok());
    }

    #[test]
    fn verify_ed25519_rejects_forgery_under_low_order_key() {
        // R = identity, s = 0 satisfies the cofactored/plain equation for a
        // small-order public key on many messages; verify_strict must refuse.
        let mut identity = [0u8; 32];
        identity[0] = 1;
        let mut sig = [0u8; 64];
        sig[0] = 1; // R = identity encoding, s = 0
        for msg in [&b"a"[..], b"b", b"c", b"d", b"forged index"] {
            assert!(verify_ed25519(msg, &hex::encode(sig), &identity).is_err());
            assert!(verify_ed25519(msg, &hex::encode(sig), &ZERO_PUBKEY).is_err());
        }
    }

    #[test]
    fn plugin_ids_are_validated_as_path_components() {
        for ok in ["mega-nz", "a", "0x", "real-debrid"] {
            assert!(validate_plugin_id(ok).is_ok(), "{ok}");
        }
        let long = "a".repeat(65);
        for bad in [
            "",
            "../evil",
            "..",
            "a/b",
            "a\\b",
            "-lead",
            "Upper",
            "dot.ted",
            "sp ace",
            long.as_str(),
        ] {
            assert!(validate_plugin_id(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn non_source_artifacts_are_rejected() {
        assert_eq!(
            source_extension("https://x.test/p/plugin.ts").unwrap(),
            "ts"
        );
        assert_eq!(
            source_extension("https://x.test/p/plugin.js?raw=1").unwrap(),
            "js"
        );
        for bad in [
            "https://x.test/p/plugin.qjsc",
            "https://x.test/p/plugin.bin",
            "https://x.test/p/plugin.ts.gz",
            "https://x.test/p/",
        ] {
            assert!(source_extension(bad).is_err(), "{bad}");
        }
    }

    /// Public half of `SigningKey::from_bytes(&[5u8; 32])`, pinned as a
    /// constant so the config under test is built from public data only.
    const TEST_INDEX_VERIFIER: [u8; 32] =
        parse_hex32("6e7a1cdd29b0b78fd13af4c5598feff4ef2a97166e3ca6f2e4fbfccd80505bf1");

    fn signed_index(signer: &ed25519_dalek::SigningKey, plugins: &str) -> (Vec<u8>, String) {
        use ed25519_dalek::Signer;
        let body = format!(r#"{{"schema_version":1,"plugins":[{plugins}]}}"#).into_bytes();
        let sig = hex::encode(signer.sign(&body).to_bytes());
        (body, sig)
    }

    #[tokio::test]
    async fn cached_index_is_reverified_and_tampering_forces_a_refetch() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let signer = ed25519_dalek::SigningKey::from_bytes(&[5u8; 32]);
        assert_eq!(signer.verifying_key().to_bytes(), TEST_INDEX_VERIFIER);
        let (body, sig) = signed_index(&signer, "");
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/index.json"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
            .expect(2)
            .mount(&mock)
            .await;
        Mock::given(method("GET"))
            .and(path("/index.json.sig"))
            .respond_with(ResponseTemplate::new(200).set_body_string(sig))
            .mount(&mock)
            .await;

        let tmp = tempfile::tempdir().unwrap();
        let cache_path = tmp.path().join("index.json");
        let cfg = RegistryConfig {
            index_url: format!("{}/index.json", mock.uri()),
            cache_path: Some(cache_path.clone()),
            ..RegistryConfig::default()
        }
        .with_index_verifier(TEST_INDEX_VERIFIER)
        .unwrap();
        let client = reqwest::Client::new();

        // 1st load fetches and caches the exact signed bytes + signature.
        load_index(&client, &cfg).await.unwrap();
        assert_eq!(std::fs::read(&cache_path).unwrap(), body);
        assert!(cached_signature_path(&cache_path).exists());

        // 2nd load is served from the (verified) cache — no fetch.
        load_index(&client, &cfg).await.unwrap();

        // Tamper with the cache: inject a plugin entry. The signature no
        // longer verifies, so this is a miss and the index is refetched.
        let forged = br#"{"schema_version":1,"plugins":[{"id":"evil","name":"e","version":"9.9.9","description":"","author":"","url_pattern":".*","min_app_version":null,"sha256":"00","download_url":"https://evil.test/p.ts","tags":[]}]}"#;
        std::fs::write(&cache_path, forged).unwrap();
        let index = load_index(&client, &cfg).await.unwrap();
        assert!(
            index.plugins.is_empty(),
            "forged cache entry must not be used"
        );
        assert_eq!(std::fs::read(&cache_path).unwrap(), body, "cache rewritten");
        // `expect(2)` on the index mock is verified when `mock` drops.
    }

    #[tokio::test]
    async fn download_rejects_traversal_ids_and_oversized_artifacts() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock = MockServer::start().await;
        let big = vec![b'a'; MAX_PLUGIN_ARTIFACT_BYTES + 1];
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(big.clone()))
            .mount(&mock)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let client = reqwest::Client::new();

        let mut p = plugin_with_pattern("../../escape", ".*");
        p.download_url = format!("{}/p.ts", mock.uri());
        let err = download_plugin(&client, &p, tmp.path()).await.unwrap_err();
        assert!(err.to_string().contains("invalid plugin id"), "{err}");
        assert!(!tmp.path().parent().unwrap().join("escape").exists());

        let mut p = plugin_with_pattern("big", ".*");
        p.download_url = format!("{}/p.ts", mock.uri());
        p.sha256 = hex::encode(Sha256::digest(&big));
        let err = download_plugin(&client, &p, tmp.path()).await.unwrap_err();
        assert!(err.to_string().contains("limit"), "{err}");
        assert!(!tmp.path().join("big").exists());
    }

    #[test]
    fn incompatible_and_widening_entries_in_update_check() {
        let mut newer = plugin_with_pattern("p", ".*");
        newer.version = "2.0.0".into();
        newer.permissions = Some(crate::types::PluginPermissions {
            domains: Some(vec!["a.com".into(), "b.com".into()]),
        });
        let mut future = plugin_with_pattern("f", ".*");
        future.api_version = Some(crate::HOST_API_VERSION + 1);
        let index = RegistryIndex {
            schema_version: 1,
            plugins: vec![newer, future],
        };
        let installed = vec![PluginMeta {
            id: "p".into(),
            name: "p".into(),
            version: "1.0.0".into(),
            url_pattern: ".*".into(),
            file_path: "/tmp/p.ts".into(),
            enabled: true,
            description: None,
            author: None,
            plugin_type: Default::default(),
            api_version: 1,
            permissions: crate::types::PluginPermissions {
                domains: Some(vec!["a.com".into()]),
            },
        }];
        let updates = check_plugin_updates(&index, &installed);
        assert_eq!(updates.len(), 1, "incompatible plugin must not be offered");
        assert!(updates[0].requires_approval);
        assert_eq!(
            updates[0].added_domains.as_deref(),
            Some(&["b.com".to_string()][..])
        );
    }
}
