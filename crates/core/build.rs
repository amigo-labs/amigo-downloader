//! Build-time guards for the core crate.
//!
//! The self-updater trusts whatever Ed25519 public key is baked into the
//! binary at compile time (see `updater::AMIGO_UPDATE_PUBLIC_KEY`). For
//! release builds we refuse to compile without `AMIGO_UPDATE_PUBKEY_HEX`
//! pointing at a real 32-byte key, otherwise an opaque "all zeros" key would
//! ship and any third party with the matching all-zero private key could
//! forge a signed self-update (remote code execution).
//!
//! Dev / debug / test builds may still compile without the env var; the
//! runtime falls back to "signature verification disabled" with a loud
//! warning (SHA256 is still enforced).

fn main() {
    println!("cargo:rerun-if-env-changed=AMIGO_UPDATE_PUBKEY_HEX");
    println!("cargo:rerun-if-env-changed=PROFILE");

    let profile = std::env::var("PROFILE").unwrap_or_default();
    let pubkey_hex = std::env::var("AMIGO_UPDATE_PUBKEY_HEX").ok();

    if profile == "release" {
        let Some(hex) = pubkey_hex else {
            panic!(
                "AMIGO_UPDATE_PUBKEY_HEX must be set for release builds. \
                 Set it to the hex-encoded 32-byte Ed25519 public key of the \
                 self-update signer (64 lowercase hex chars)."
            );
        };
        if hex.len() != 64 {
            panic!(
                "AMIGO_UPDATE_PUBKEY_HEX must be exactly 64 hex chars, got {}",
                hex.len()
            );
        }
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            panic!("AMIGO_UPDATE_PUBKEY_HEX must be hex (0-9, a-f, A-F) only");
        }
        let zero = "0".repeat(64);
        if hex.eq_ignore_ascii_case(&zero) {
            panic!(
                "AMIGO_UPDATE_PUBKEY_HEX is the all-zero placeholder; \
                 release builds must pin a real Ed25519 public key."
            );
        }
    }
}
