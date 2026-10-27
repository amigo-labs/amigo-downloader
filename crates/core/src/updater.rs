//! Core binary self-update via GitHub Releases.
//!
//! Checks for new releases, downloads the release archive of the running
//! product for the current platform, verifies its signature and SHA256,
//! extracts the binaries and performs an atomic self-replace. See
//! `docs/specs/self-update.md`.

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

/// Current app version from Cargo.toml.
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Ed25519 public key of the amigo-labs self-update signer.
///
/// Injected at compile time from the `AMIGO_UPDATE_PUBKEY_HEX` environment
/// variable (64 hex chars). When unset the value falls back to the all-zero
/// placeholder, which [`trusted_update_key`] treats as "verification
/// disabled" (with a runtime warning) so dev builds don't trust a forgeable
/// key. `build.rs` refuses to compile a release build when the env var is
/// unset, so production binaries always pin a real key. Rotate by shipping a
/// new release; clients that haven't updated will reject the new signatures,
/// which is the safer failure mode.
pub const AMIGO_UPDATE_PUBLIC_KEY: [u8; 32] = update_pubkey_from_env();

const ZERO_PUBKEY: [u8; 32] = [0u8; 32];

const fn update_pubkey_from_env() -> [u8; 32] {
    match option_env!("AMIGO_UPDATE_PUBKEY_HEX") {
        Some(hex) => parse_hex32(hex),
        None => ZERO_PUBKEY,
    }
}

const fn parse_hex32(s: &str) -> [u8; 32] {
    let bytes = s.as_bytes();
    assert!(
        bytes.len() == 64,
        "AMIGO_UPDATE_PUBKEY_HEX must be 64 hex chars (32-byte Ed25519 public key)"
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
        _ => panic!("AMIGO_UPDATE_PUBKEY_HEX contains non-hex character"),
    }
}

/// The Ed25519 key the self-update must be signed with, or `None` when no real
/// key was pinned at compile time (dev builds only). Disabling verification
/// against the zero placeholder is safer than trusting a key whose private
/// half is publicly derivable.
pub fn trusted_update_key() -> Option<[u8; 32]> {
    if AMIGO_UPDATE_PUBLIC_KEY == ZERO_PUBKEY {
        None
    } else {
        Some(AMIGO_UPDATE_PUBLIC_KEY)
    }
}

/// Verify that `payload` carries the Ed25519 signature `sig_hex` produced by
/// the private key matching `pubkey`.
pub fn verify_ed25519(
    payload: &[u8],
    sig_hex: &str,
    pubkey: &[u8; 32],
) -> Result<(), crate::Error> {
    let sig_bytes = hex::decode(sig_hex.trim())
        .map_err(|e| crate::Error::Update(format!("update signature is not hex: {e}")))?;
    if sig_bytes.len() != 64 {
        return Err(crate::Error::Update(format!(
            "update signature has wrong length {} (want 64)",
            sig_bytes.len()
        )));
    }
    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig_bytes);
    let sig = Signature::from_bytes(&sig_arr);
    let vk = VerifyingKey::from_bytes(pubkey)
        .map_err(|e| crate::Error::Update(format!("bad update pubkey: {e}")))?;
    vk.verify_strict(payload, &sig).map_err(|_| {
        crate::Error::Update("update signature did not verify against the trusted key".into())
    })
}

/// Reject update URLs that don't point at GitHub's release-hosting domains.
/// This blocks an attacker who can influence the release metadata from
/// pointing the download / checksum / signature at an arbitrary host, and
/// requires HTTPS so metadata can't be served over plain HTTP.
fn validate_update_host(url: &str) -> Result<(), crate::Error> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|e| crate::Error::Update(format!("malformed update URL {url:?}: {e}")))?;
    if parsed.scheme() != "https" {
        return Err(crate::Error::Update(format!(
            "refusing non-HTTPS update URL {url:?}"
        )));
    }
    let host = parsed.host_str().unwrap_or_default();
    let trusted = host == "github.com"
        || host == "api.github.com"
        || host == "codeload.github.com"
        || host == "objects.githubusercontent.com"
        || host.ends_with(".githubusercontent.com")
        || host.ends_with(".github.com");
    if !trusted {
        return Err(crate::Error::Update(format!(
            "refusing update from untrusted host {host:?}"
        )));
    }
    Ok(())
}

/// Reject a target version that is not strictly newer than the running one.
/// Guards the apply path against rollback to a known-vulnerable release even
/// if the release metadata (or a MITM) announces an older version.
fn verify_upgrade_only(target: &str) -> Result<(), crate::Error> {
    let current =
        semver::Version::parse(CURRENT_VERSION).unwrap_or_else(|_| semver::Version::new(0, 0, 0));
    let target_v = semver::Version::parse(target.trim_start_matches('v')).map_err(|e| {
        crate::Error::Update(format!(
            "update version {target:?} is not valid semver: {e}"
        ))
    })?;
    if target_v <= current {
        return Err(crate::Error::Update(format!(
            "refusing downgrade: target {target_v} is not newer than current {current}"
        )));
    }
    Ok(())
}

/// How the app is distributed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Distribution {
    Cli,
    Server,
    Docker,
    Tauri,
}

/// Name of the plugin host binary that ships next to the CLI and the server.
pub const PLUGIN_HOST_NAME: &str = "amigo-plugin-host";

/// The release product a binary updates itself from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Product {
    /// `amigo-server`
    Server,
    /// `amigo-dl`
    Cli,
}

impl Product {
    pub fn binary_name(self) -> &'static str {
        match self {
            Product::Server => "amigo-server",
            Product::Cli => "amigo-dl",
        }
    }

    /// Release archive of this product for the running platform, e.g.
    /// `amigo-dl-linux-x86_64.tar.gz`.
    pub fn archive_name(self) -> String {
        archive_name(self, std::env::consts::OS, std::env::consts::ARCH)
    }
}

fn archive_name(product: Product, os: &str, arch: &str) -> String {
    let ext = if os == "windows" { "zip" } else { "tar.gz" };
    format!("{}-{os}-{arch}.{ext}", product.binary_name())
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
        sig_url: Option<String>,
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
    product: Product,
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
    let Ok(latest) = semver::Version::parse(latest_tag) else {
        debug!("Could not parse release tag as semver: {latest_tag}");
        return Ok(CoreUpdateStatus::UpToDate);
    };

    if latest > current {
        let dist = detect_distribution();
        let asset = select_asset(&release, product);
        let can_self_update =
            (dist == Distribution::Cli || dist == Distribution::Server) && asset.is_some();

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
        let sig_url = asset.and_then(|a| {
            let sig_name = format!("{}.sig", a.name);
            release
                .assets
                .iter()
                .find(|x| x.name == sig_name)
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
            sig_url,
        })
    } else {
        debug!("Up to date: {CURRENT_VERSION}");
        Ok(CoreUpdateStatus::UpToDate)
    }
}

/// Select the release archive of `product` for the current platform. Only
/// the exact archive name matches, so the sibling `.sha256` / `.sig` assets
/// and the other product's archive are never picked.
pub fn select_asset(release: &ReleaseInfo, product: Product) -> Option<&ReleaseAsset> {
    let target = product.archive_name();
    debug!("Looking for asset: {target}");
    release.assets.iter().find(|a| a.name == target)
}

/// Download a release asset and verify it before returning the temp path.
///
/// Verification is layered: an Ed25519 signature over the archive (when a
/// key is pinned into the build) proves it was produced by the publisher,
/// and a SHA256 checksum guards integrity. Both must pass. The signature is
/// the trust anchor — SHA256 alone only binds the archive to whoever served
/// the release, not to the publisher.
pub async fn download_and_verify(
    client: &reqwest::Client,
    asset: &ReleaseAsset,
    sha256_url: Option<&str>,
    sig_url: Option<&str>,
) -> Result<PathBuf, crate::Error> {
    download_and_verify_with_key(client, asset, sha256_url, sig_url, trusted_update_key()).await
}

/// Core of [`download_and_verify`] with the trusted signing key injected, so
/// tests can exercise signature verification without a compile-time key.
async fn download_and_verify_with_key(
    client: &reqwest::Client,
    asset: &ReleaseAsset,
    sha256_url: Option<&str>,
    sig_url: Option<&str>,
    signing_key: Option<[u8; 32]>,
) -> Result<PathBuf, crate::Error> {
    // A checksum is mandatory: self-replacing the running binary with an
    // unverified download would let anyone able to influence the release (or
    // the resolved `github_repo`) achieve code execution. Bail before we even
    // fetch the binary if no `.sha256` asset accompanies it.
    let sha_url = sha256_url.ok_or_else(|| {
        crate::Error::Update(
            "refusing to apply update: release has no .sha256 checksum asset".into(),
        )
    })?;

    // Pin the hosts of every URL we're about to fetch to GitHub's release
    // domains so tampered metadata can't redirect us to an attacker host.
    validate_update_host(&asset.browser_download_url)?;
    validate_update_host(sha_url)?;

    // Signature verification. When a real key is pinned (release builds), a
    // detached `.sig` asset is mandatory and must verify against the binary.
    // Dev builds with no pinned key fall back to SHA256-only with a warning.
    if signing_key.is_some() {
        let sig_url = sig_url.ok_or_else(|| {
            crate::Error::Update(
                "refusing to apply update: release has no .sig signature asset".into(),
            )
        })?;
        validate_update_host(sig_url)?;
    }

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

    // Verify the Ed25519 signature over the downloaded bytes before anything
    // else — it's the strongest check and binds the archive to the publisher.
    match signing_key {
        Some(key) => {
            let sig_url = sig_url.expect("sig_url presence checked above when key is pinned");
            let sig_hex = client
                .get(sig_url)
                .send()
                .await
                .map_err(|e| crate::Error::Update(format!("Signature download failed: {e}")))?
                .error_for_status()
                .map_err(|e| crate::Error::Update(format!("Signature download failed: {e}")))?
                .text()
                .await
                .map_err(|e| crate::Error::Update(format!("Signature read failed: {e}")))?;
            verify_ed25519(&bytes, &sig_hex, &key)?;
            debug!("Ed25519 update signature verified");
        }
        None => warn!(
            "Update signature verification DISABLED (no key pinned) — only safe for local development"
        ),
    }

    // Verify SHA256 (mandatory, secondary integrity check).
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

    if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(crate::Error::Update(
            "checksum asset did not contain a valid SHA256 hex digest".into(),
        ));
    }

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual = hex::encode(hasher.finalize());

    if actual != expected {
        return Err(crate::Error::ChecksumMismatch);
    }
    debug!("SHA256 verified: {actual}");

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

/// Largest binary accepted from an update archive.
const MAX_BINARY_BYTES: u64 = 512 * 1024 * 1024;

/// Install a downloaded, verified release archive of `product` next to the
/// running executable. Returns Ok(()) — a restart is needed to run the new
/// version.
pub fn apply_update(archive: &Path, product: Product) -> Result<(), crate::Error> {
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

    let exe = std::env::current_exe()
        .and_then(fs::canonicalize)
        .map_err(|e| crate::Error::Update(format!("cannot locate the running executable: {e}")))?;
    let dir = exe.parent().ok_or_else(|| {
        crate::Error::Update(format!("{} has no parent directory", exe.display()))
    })?;

    info!("Applying self-update from {:?} to {:?}", archive, dir);
    // Stage next to the executable so the installs below are same-filesystem
    // renames.
    let staging = tempfile::Builder::new()
        .prefix(".amigo-update-")
        .tempdir_in(dir)
        .map_err(|e| crate::Error::Update(format!("cannot stage the update in {dir:?}: {e}")))?;
    let staged = extract_update(archive, staging.path(), product)?;
    install(&staged, dir, |binary| self_replace::self_replace(binary))?;

    info!("Update applied successfully — restart to use new version");
    Ok(())
}

/// Binaries extracted from an update archive.
#[derive(Debug)]
struct Staged {
    binary: PathBuf,
    plugin_host: Option<PathBuf>,
}

fn plugin_host_file_name() -> String {
    format!("{PLUGIN_HOST_NAME}{}", std::env::consts::EXE_SUFFIX)
}

/// Extract the product binary and, when present, the plugin host from
/// `archive` into `dest`. Nothing but those two names is ever written.
fn extract_update(archive: &Path, dest: &Path, product: Product) -> Result<Staged, crate::Error> {
    let binary_name = format!("{}{}", product.binary_name(), std::env::consts::EXE_SUFFIX);
    let host_name = plugin_host_file_name();
    let wanted = [binary_name.as_str(), host_name.as_str()];

    let mut file = File::open(archive)?;
    let mut magic = [0u8; 4];
    let read = file.read(&mut magic)?;
    file.seek(SeekFrom::Start(0))?;

    let mut written = Vec::new();
    if read >= 2 && magic[..2] == [0x1f, 0x8b] {
        extract_tar_gz(file, dest, &wanted, &mut written)?;
    } else if read == 4 && magic == *b"PK\x03\x04" {
        extract_zip(file, dest, &wanted, &mut written)?;
    } else {
        return Err(crate::Error::Update(
            "update is neither a .tar.gz nor a .zip archive".into(),
        ));
    }

    if !written.contains(&binary_name) {
        return Err(crate::Error::Update(format!(
            "update archive does not contain {binary_name}"
        )));
    }
    Ok(Staged {
        binary: dest.join(&binary_name),
        plugin_host: written.contains(&host_name).then(|| dest.join(&host_name)),
    })
}

/// The whitelisted name `path` refers to, if it is a single file name at
/// the archive root (an optional `./` prefix is allowed).
fn wanted_name<'a>(path: &Path, wanted: &[&'a str]) -> Option<&'a str> {
    let mut normal = path.components().filter(|c| *c != Component::CurDir);
    let Some(Component::Normal(name)) = normal.next() else {
        return None;
    };
    if normal.next().is_some() {
        return None;
    }
    wanted.iter().copied().find(|w| name == *w)
}

fn extract_tar_gz(
    file: File,
    dest: &Path,
    wanted: &[&str],
    written: &mut Vec<String>,
) -> Result<(), crate::Error> {
    let bad = |e: io::Error| crate::Error::Update(format!("corrupt update archive: {e}"));
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    for entry in archive.entries().map_err(bad)? {
        let entry = entry.map_err(bad)?;
        let path = entry.path().map_err(bad)?.into_owned();
        let Some(name) = wanted_name(&path, wanted) else {
            debug!("Skipping update archive entry {path:?}");
            continue;
        };
        if !entry.header().entry_type().is_file() {
            return Err(crate::Error::Update(format!(
                "{name} in the update archive is not a regular file"
            )));
        }
        write_binary(entry, dest, name, written)?;
    }
    Ok(())
}

fn extract_zip(
    file: File,
    dest: &Path,
    wanted: &[&str],
    written: &mut Vec<String>,
) -> Result<(), crate::Error> {
    let bad =
        |e: zip::result::ZipError| crate::Error::Update(format!("corrupt update archive: {e}"));
    let mut archive = zip::ZipArchive::new(file).map_err(bad)?;
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(bad)?;
        let Some(name) = entry
            .enclosed_name()
            .and_then(|path| wanted_name(&path, wanted))
        else {
            debug!("Skipping update archive entry {:?}", entry.name());
            continue;
        };
        if !entry.is_file() || entry.is_symlink() {
            return Err(crate::Error::Update(format!(
                "{name} in the update archive is not a regular file"
            )));
        }
        write_binary(entry, dest, name, written)?;
    }
    Ok(())
}

fn write_binary(
    reader: impl Read,
    dest: &Path,
    name: &str,
    written: &mut Vec<String>,
) -> Result<(), crate::Error> {
    if written.iter().any(|n| n == name) {
        return Err(crate::Error::Update(format!(
            "update archive contains {name} more than once"
        )));
    }
    let path = dest.join(name);
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    let copied = io::copy(&mut reader.take(MAX_BINARY_BYTES + 1), &mut out)?;
    if copied > MAX_BINARY_BYTES {
        return Err(crate::Error::Update(format!(
            "{name} in the update archive exceeds {MAX_BINARY_BYTES} bytes"
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
    }
    written.push(name.to_string());
    Ok(())
}

/// Install `staged` into `dir`: the plugin host first, then the product
/// binary through `replace_binary`. If that fails, the plugin host is rolled
/// back so the pair on disk stays consistent.
fn install(
    staged: &Staged,
    dir: &Path,
    replace_binary: impl FnOnce(&Path) -> io::Result<()>,
) -> Result<(), crate::Error> {
    let host_target = dir.join(plugin_host_file_name());
    let backup = dir.join(format!("{PLUGIN_HOST_NAME}.old"));

    let mut had_previous = false;
    if let Some(new_host) = &staged.plugin_host {
        // Left by an earlier update when Windows could not delete it while
        // an old plugin host was still running.
        if backup.exists()
            && let Err(e) = fs::remove_file(&backup)
        {
            return Err(crate::Error::Update(format!(
                "cannot remove {backup:?} from an earlier update (restart and retry): {e}"
            )));
        }
        if host_target.exists() {
            fs::rename(&host_target, &backup).map_err(|e| {
                crate::Error::Update(format!("cannot move the current plugin host aside: {e}"))
            })?;
            had_previous = true;
        }
        if let Err(e) = fs::rename(new_host, &host_target) {
            if had_previous && let Err(e) = fs::rename(&backup, &host_target) {
                warn!("Could not restore the previous plugin host from {backup:?}: {e}");
            }
            return Err(crate::Error::Update(format!(
                "cannot install the new plugin host: {e}"
            )));
        }
    }

    if let Err(e) = replace_binary(&staged.binary) {
        if staged.plugin_host.is_some() {
            let rollback = if had_previous {
                fs::rename(&backup, &host_target)
            } else {
                fs::remove_file(&host_target)
            };
            if let Err(e) = rollback {
                warn!("Could not roll back the plugin host at {host_target:?}: {e}");
            }
        }
        return Err(crate::Error::Update(format!("Self-replace failed: {e}")));
    }

    if had_previous && let Err(e) = fs::remove_file(&backup) {
        // Windows: an old plugin host may still be running from it.
        debug!("Left {backup:?} for the next update: {e}");
    }
    Ok(())
}

/// Download, verify, and apply an update in one step.
///
/// `expected_version` is the version announced by the release check; it is
/// re-verified to be strictly newer than the running binary here, at apply
/// time, so the fire-and-forget apply path can't be tricked into installing a
/// downgrade even if the check-time gate was bypassed.
pub async fn download_and_apply(
    client: &reqwest::Client,
    download_url: &str,
    sha256_url: Option<&str>,
    sig_url: Option<&str>,
    expected_version: &str,
    product: Product,
) -> Result<(), crate::Error> {
    verify_upgrade_only(expected_version)?;
    let asset = ReleaseAsset {
        name: product.archive_name(),
        browser_download_url: download_url.to_string(),
        size: 0,
    };
    let path = download_and_verify(client, &asset, sha256_url, sig_url).await?;
    let applied = tokio::task::spawn_blocking({
        let path = path.clone();
        move || apply_update(&path, product)
    })
    .await
    .map_err(|e| crate::Error::Update(format!("update task failed: {e}")));
    // Clean up temp file
    let _ = fs::remove_file(&path);
    applied?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches;

    #[test]
    fn test_detect_distribution_default() {
        // Clean env for this test
        // SAFETY: Test runs single-threaded via cargo test -- --test-threads=1
        // or env vars are not shared across these specific tests in practice.
        unsafe {
            std::env::remove_var("AMIGO_DOCKER");
            std::env::remove_var("AMIGO_TAURI");
        }
        let dist = detect_distribution();
        assert_eq!(dist, Distribution::Server);
    }

    #[test]
    fn test_detect_distribution_docker() {
        unsafe {
            std::env::set_var("AMIGO_DOCKER", "1");
            std::env::remove_var("AMIGO_TAURI");
        }
        let dist = detect_distribution();
        assert_eq!(dist, Distribution::Docker);
        unsafe {
            std::env::remove_var("AMIGO_DOCKER");
        }
    }

    #[test]
    fn test_detect_distribution_tauri() {
        unsafe {
            std::env::remove_var("AMIGO_DOCKER");
            std::env::set_var("AMIGO_TAURI", "1");
        }
        let dist = detect_distribution();
        assert_eq!(dist, Distribution::Tauri);
        unsafe {
            std::env::remove_var("AMIGO_TAURI");
        }
    }

    #[test]
    fn test_detect_distribution_docker_takes_precedence() {
        // Docker should take precedence over Tauri if both are set
        unsafe {
            std::env::set_var("AMIGO_DOCKER", "1");
            std::env::set_var("AMIGO_TAURI", "1");
        }
        let dist = detect_distribution();
        assert_eq!(dist, Distribution::Docker);
        unsafe {
            std::env::remove_var("AMIGO_DOCKER");
            std::env::remove_var("AMIGO_TAURI");
        }
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

    fn asset(name: &str) -> ReleaseAsset {
        ReleaseAsset {
            name: name.into(),
            browser_download_url: format!("https://github.com/x/{name}"),
            size: 1,
        }
    }

    #[test]
    fn select_asset_picks_the_products_archive() {
        let server = Product::Server.archive_name();
        let cli = Product::Cli.archive_name();
        // Siblings and the other product come first, so a prefix match or a
        // first-match on the platform suffix would pick the wrong asset.
        let release = ReleaseInfo {
            tag_name: "v0.2.0".into(),
            name: None,
            body: None,
            assets: vec![
                asset(&format!("{server}.sig")),
                asset(&format!("{server}.sha256")),
                asset(&format!("{cli}.sig")),
                asset(&server),
                asset(&cli),
            ],
        };

        assert_eq!(
            select_asset(&release, Product::Server).unwrap().name,
            server
        );
        assert_eq!(select_asset(&release, Product::Cli).unwrap().name, cli);

        let server_only = ReleaseInfo {
            assets: vec![asset(&server)],
            ..release
        };
        assert!(select_asset(&server_only, Product::Cli).is_none());
    }

    #[test]
    fn archive_names_match_the_release_workflow() {
        assert_eq!(
            archive_name(Product::Server, "linux", "x86_64"),
            "amigo-server-linux-x86_64.tar.gz"
        );
        assert_eq!(
            archive_name(Product::Cli, "macos", "aarch64"),
            "amigo-dl-macos-aarch64.tar.gz"
        );
        assert_eq!(
            archive_name(Product::Cli, "windows", "x86_64"),
            "amigo-dl-windows-x86_64.zip"
        );
    }

    #[tokio::test]
    async fn test_download_refuses_without_checksum() {
        // A release with no .sha256 asset must NOT be applied unverified. The
        // check happens before any network I/O, so we can assert it offline.
        let asset = ReleaseAsset {
            name: "amigo-server".into(),
            browser_download_url: "https://github.com/x/never-fetched".into(),
            size: 0,
        };
        let client = reqwest::Client::new();
        let err = download_and_verify(&client, &asset, None, None)
            .await
            .expect_err("must refuse to apply an unverified update");
        assert_matches!(
            err,
            crate::Error::Update(_),
            "expected an Update error, got {err:?}"
        );
    }

    // ---- Ed25519 signature verification (the trust anchor for #30) ----

    fn test_keypair() -> (ed25519_dalek::SigningKey, [u8; 32]) {
        // Deterministic key from a fixed seed — no rand_core feature needed.
        let seed = [7u8; 32];
        let sk = ed25519_dalek::SigningKey::from_bytes(&seed);
        let pk = sk.verifying_key().to_bytes();
        (sk, pk)
    }

    #[test]
    fn verify_ed25519_accepts_valid_signature() {
        use ed25519_dalek::Signer;
        let (sk, pk) = test_keypair();
        let payload = b"amigo-server binary bytes";
        let sig_hex = hex::encode(sk.sign(payload).to_bytes());
        verify_ed25519(payload, &sig_hex, &pk).expect("valid signature must verify");
    }

    #[test]
    fn verify_ed25519_rejects_tampered_payload() {
        use ed25519_dalek::Signer;
        let (sk, pk) = test_keypair();
        let sig_hex = hex::encode(sk.sign(b"original binary").to_bytes());
        let err = verify_ed25519(b"tampered binary", &sig_hex, &pk)
            .expect_err("tampered payload must be rejected");
        assert_matches!(err, crate::Error::Update(_));
    }

    #[test]
    fn verify_ed25519_rejects_wrong_key() {
        use ed25519_dalek::Signer;
        let (sk, _pk) = test_keypair();
        let payload = b"binary";
        let sig_hex = hex::encode(sk.sign(payload).to_bytes());
        // A different key must not validate the signature.
        let other = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32])
            .verifying_key()
            .to_bytes();
        let err =
            verify_ed25519(payload, &sig_hex, &other).expect_err("wrong key must be rejected");
        assert_matches!(err, crate::Error::Update(_));
    }

    #[test]
    fn verify_ed25519_rejects_malformed_signature() {
        let (_sk, pk) = test_keypair();
        assert!(verify_ed25519(b"x", "not-hex!!", &pk).is_err());
        assert!(
            verify_ed25519(b"x", "abcd", &pk).is_err(),
            "short sig must fail"
        );
    }

    #[test]
    fn signatures_from_the_release_script_verify() {
        // Produced by `scripts/sign-update-archives.sh` with the seed of
        // `test_keypair` over a file holding exactly this payload. Pins the
        // script's output format (hex of the raw 64-byte signature) to what
        // the updater verifies.
        let (_sk, pk) = test_keypair();
        let sig = "ae059b1733a2e7d7dd4ec8a45c11d2d461593784b7ae84c6c1319b76889c0b65\
                   fe6774a7374e460216a2ed88313088d7288171d2c6b9410d7673b88fb6ba5609";
        verify_ed25519(b"amigo update archive fixture", sig, &pk)
            .expect("script signature must verify");
    }

    // ---- Host pinning ----

    #[test]
    fn host_pinning_accepts_github_hosts() {
        for url in [
            "https://github.com/amigo-labs/amigo-downloader/releases/download/v1/bin",
            "https://objects.githubusercontent.com/foo",
            "https://codeload.github.com/foo",
            "https://api.github.com/repos/x/releases",
        ] {
            validate_update_host(url).unwrap_or_else(|e| panic!("{url} should be allowed: {e}"));
        }
    }

    #[test]
    fn host_pinning_rejects_untrusted_and_plain_http() {
        assert!(validate_update_host("https://evil.example.com/bin").is_err());
        assert!(
            validate_update_host("http://github.com/x").is_err(),
            "plain HTTP must be rejected"
        );
        // Look-alike host must not slip through the suffix check.
        assert!(validate_update_host("https://github.com.evil.com/x").is_err());
    }

    // ---- Anti-downgrade ----

    #[test]
    fn upgrade_check_rejects_downgrade_and_same_version() {
        assert!(
            verify_upgrade_only("0.0.1").is_err(),
            "older version must be rejected"
        );
        assert!(
            verify_upgrade_only(CURRENT_VERSION).is_err(),
            "same version must be rejected"
        );
        assert!(verify_upgrade_only("v999.0.0").is_ok(), "newer must pass");
        assert!(verify_upgrade_only("not-semver").is_err());
    }

    #[tokio::test]
    async fn download_refuses_when_key_pinned_but_no_signature() {
        // With a signing key pinned, a release lacking a .sig asset must be
        // refused before any binary is fetched. Using github.com URLs keeps
        // host pinning happy; the missing-sig check trips first.
        let (_sk, pk) = test_keypair();
        let asset = ReleaseAsset {
            name: "amigo-server".into(),
            browser_download_url: "https://github.com/x/bin".into(),
            size: 0,
        };
        let client = reqwest::Client::new();
        let err = download_and_verify_with_key(
            &client,
            &asset,
            Some("https://github.com/x/bin.sha256"),
            None,
            Some(pk),
        )
        .await
        .expect_err("must refuse when signature is missing");
        assert_matches!(err, crate::Error::Update(m) if m.contains(".sig"),
            "error should mention the missing signature asset");
    }

    #[tokio::test]
    async fn download_rejects_untrusted_download_host() {
        let asset = ReleaseAsset {
            name: "amigo-server".into(),
            browser_download_url: "https://evil.example.com/bin".into(),
            size: 0,
        };
        let client = reqwest::Client::new();
        let err = download_and_verify_with_key(
            &client,
            &asset,
            Some("https://github.com/x/bin.sha256"),
            None,
            None,
        )
        .await
        .expect_err("must reject a non-GitHub download host");
        assert_matches!(err, crate::Error::Update(_));
    }

    // ---- Archive extraction ----

    fn exe(stem: &str) -> String {
        format!("{stem}{}", std::env::consts::EXE_SUFFIX)
    }

    fn tar_header(name: &str, size: usize, kind: tar::EntryType) -> tar::Header {
        let mut header = tar::Header::new_gnu();
        header.set_size(size as u64);
        header.set_mode(0o755);
        header.set_entry_type(kind);
        // Raw name, so tests can also write `../` paths that `set_path` rejects.
        header.as_old_mut().name[..name.len()].copy_from_slice(name.as_bytes());
        header.set_cksum();
        header
    }

    fn tar_gz(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::fast(),
        ));
        for (name, data) in entries {
            let header = tar_header(name, data.len(), tar::EntryType::Regular);
            builder.append(&header, *data).unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap()
    }

    fn zip_archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut writer = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
        for (name, data) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    /// Extract `bytes` as an `amigo-dl` update. The staging directory is
    /// `<tmp>/staging`, so a `../evil` entry would land at `<tmp>/evil`.
    fn extract(bytes: &[u8]) -> (tempfile::TempDir, Result<Staged, crate::Error>) {
        let tmp = tempfile::tempdir().unwrap();
        let archive = tmp.path().join("update");
        fs::write(&archive, bytes).unwrap();
        let dest = tmp.path().join("staging");
        fs::create_dir(&dest).unwrap();
        let staged = extract_update(&archive, &dest, Product::Cli);
        (tmp, staged)
    }

    fn staged_names(tmp: &tempfile::TempDir) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(tmp.path().join("staging"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn extracts_binaries_from_tar_gz() {
        let bin = exe("amigo-dl");
        let host = exe(PLUGIN_HOST_NAME);
        let (_tmp, staged) = extract(&tar_gz(&[
            (bin.as_str(), b"new-cli"),
            (host.as_str(), b"new-host"),
        ]));
        let staged = staged.unwrap();
        assert_eq!(fs::read(&staged.binary).unwrap(), b"new-cli");
        assert_eq!(
            fs::read(staged.plugin_host.as_ref().unwrap()).unwrap(),
            b"new-host"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&staged.binary).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o755);
        }
    }

    #[test]
    fn extracts_binaries_from_zip() {
        let bin = exe("amigo-dl");
        let host = exe(PLUGIN_HOST_NAME);
        let (_tmp, staged) = extract(&zip_archive(&[
            (bin.as_str(), b"new-cli"),
            (host.as_str(), b"new-host"),
        ]));
        let staged = staged.unwrap();
        assert_eq!(fs::read(&staged.binary).unwrap(), b"new-cli");
        assert_eq!(
            fs::read(staged.plugin_host.as_ref().unwrap()).unwrap(),
            b"new-host"
        );
    }

    #[test]
    fn unknown_entries_are_ignored_and_never_written() {
        let bin = exe("amigo-dl");
        let nested = format!("sub/{bin}");
        let dotted = format!("./{bin}");
        let entries: [(&str, &[u8]); 5] = [
            ("../evil", b"x"),
            (nested.as_str(), b"nested"),
            ("README.md", b"readme"),
            (&exe("amigo-server"), b"server"),
            (dotted.as_str(), b"new-cli"),
        ];

        for bytes in [tar_gz(&entries), zip_archive(&entries[1..])] {
            let (tmp, staged) = extract(&bytes);
            let staged = staged.unwrap();
            assert_eq!(fs::read(&staged.binary).unwrap(), b"new-cli");
            assert!(staged.plugin_host.is_none(), "host is optional");
            assert_eq!(staged_names(&tmp), std::slice::from_ref(&bin));
            assert!(!tmp.path().join("evil").exists());
        }

        // zip's own traversal entry: skipped via `enclosed_name`.
        let (tmp, staged) = extract(&zip_archive(&[("../evil", b"x"), (bin.as_str(), b"b")]));
        staged.unwrap();
        assert!(!tmp.path().join("evil").exists());
    }

    #[test]
    fn symlink_with_a_whitelisted_name_is_rejected() {
        let bin = exe("amigo-dl");
        let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::fast(),
        ));
        let mut header = tar_header(&bin, 0, tar::EntryType::Symlink);
        header.set_link_name("/bin/sh").unwrap();
        header.set_cksum();
        builder.append(&header, io::empty()).unwrap();
        let bytes = builder.into_inner().unwrap().finish().unwrap();

        let (tmp, staged) = extract(&bytes);
        let err = staged.unwrap_err();
        assert!(err.to_string().contains("not a regular file"), "{err}");
        assert!(staged_names(&tmp).is_empty());
    }

    #[test]
    fn duplicate_entries_are_rejected() {
        let bin = exe("amigo-dl");
        let (_tmp, staged) = extract(&tar_gz(&[(bin.as_str(), b"one"), (bin.as_str(), b"two")]));
        let err = staged.unwrap_err();
        assert!(err.to_string().contains("more than once"), "{err}");
    }

    #[test]
    fn missing_product_binary_is_an_error() {
        // The server archive does not update the CLI.
        let (_tmp, staged) = extract(&tar_gz(&[
            (exe("amigo-server").as_str(), b"server"),
            (exe(PLUGIN_HOST_NAME).as_str(), b"host"),
        ]));
        let err = staged.unwrap_err();
        assert!(err.to_string().contains("does not contain"), "{err}");
    }

    #[test]
    fn raw_binary_is_not_an_archive() {
        let (_tmp, staged) = extract(b"\x7fELF\x02\x01\x01 not an archive");
        let err = staged.unwrap_err();
        assert!(err.to_string().contains("neither"), "{err}");
    }

    // ---- Installing the pair ----

    fn stage(root: &Path, host: Option<&[u8]>) -> Staged {
        let staging = root.join("staging");
        fs::create_dir_all(&staging).unwrap();
        let binary = staging.join(exe("amigo-dl"));
        fs::write(&binary, b"new-cli").unwrap();
        let plugin_host = host.map(|bytes| {
            let path = staging.join(exe(PLUGIN_HOST_NAME));
            fs::write(&path, bytes).unwrap();
            path
        });
        Staged {
            binary,
            plugin_host,
        }
    }

    #[test]
    fn install_replaces_plugin_host_then_binary() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("bin");
        fs::create_dir(&dir).unwrap();
        let host_target = dir.join(exe(PLUGIN_HOST_NAME));
        let backup = dir.join(format!("{PLUGIN_HOST_NAME}.old"));
        fs::write(&host_target, b"old-host").unwrap();
        fs::write(&backup, b"stale backup").unwrap();
        let staged = stage(tmp.path(), Some(b"new-host"));

        install(&staged, &dir, |binary| {
            assert_eq!(fs::read(&host_target).unwrap(), b"new-host", "host first");
            fs::copy(binary, dir.join(exe("amigo-dl"))).map(|_| ())
        })
        .unwrap();

        assert_eq!(fs::read(&host_target).unwrap(), b"new-host");
        assert_eq!(fs::read(dir.join(exe("amigo-dl"))).unwrap(), b"new-cli");
        assert!(!backup.exists());
    }

    #[test]
    fn failed_binary_replace_restores_the_old_plugin_host() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("bin");
        fs::create_dir(&dir).unwrap();
        let host_target = dir.join(exe(PLUGIN_HOST_NAME));
        fs::write(&host_target, b"old-host").unwrap();
        let staged = stage(tmp.path(), Some(b"new-host"));

        let err = install(&staged, &dir, |_| Err(io::Error::other("busy"))).unwrap_err();

        assert!(err.to_string().contains("busy"), "{err}");
        assert_eq!(fs::read(&host_target).unwrap(), b"old-host");
        assert!(!dir.join(format!("{PLUGIN_HOST_NAME}.old")).exists());
    }

    #[test]
    fn failed_binary_replace_removes_a_new_plugin_host() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("bin");
        fs::create_dir(&dir).unwrap();
        let staged = stage(tmp.path(), Some(b"new-host"));

        install(&staged, &dir, |_| Err(io::Error::other("busy"))).unwrap_err();

        assert!(!dir.join(exe(PLUGIN_HOST_NAME)).exists());
    }
}
