//! Core binary self-update via GitHub Releases.
//!
//! Checks for new releases, downloads the correct binary for the current
//! platform, verifies SHA256, and performs an atomic self-replace.

use std::path::PathBuf;

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
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
    vk.verify(payload, &sig).map_err(|_| {
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

/// Download a release asset and verify it before returning the temp path.
///
/// Verification is layered: an Ed25519 signature over the binary (when a key
/// is pinned into the build) proves the binary was produced by the offline
/// publisher, and a SHA256 checksum guards integrity. Both must pass. The
/// signature is the trust anchor — SHA256 alone only binds the binary to
/// whoever served the release, not to the publisher.
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
    // else — it's the strongest check and binds the binary to the publisher.
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
) -> Result<(), crate::Error> {
    verify_upgrade_only(expected_version)?;
    let asset = ReleaseAsset {
        name: "update".into(),
        browser_download_url: download_url.to_string(),
        size: 0,
    };
    let path = download_and_verify(client, &asset, sha256_url, sig_url).await?;
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
        assert!(
            matches!(err, crate::Error::Update(_)),
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
        assert!(matches!(err, crate::Error::Update(_)));
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
        assert!(matches!(err, crate::Error::Update(_)));
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
        assert!(
            matches!(err, crate::Error::Update(m) if m.contains(".sig")),
            "error should mention the missing signature asset"
        );
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
        assert!(matches!(err, crate::Error::Update(_)));
    }
}
