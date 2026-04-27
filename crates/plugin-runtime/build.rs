//! Build-time guards for the plugin-runtime crate.
//!
//! The plugin registry trusts whatever Ed25519 public key is baked into the
//! binary at compile time (see `registry::AMIGO_REGISTRY_PUBLIC_KEY`). For
//! release builds we refuse to compile without `AMIGO_REGISTRY_PUBKEY_HEX`
//! pointing at a real 32-byte key, otherwise an opaque "all zeros" key would
//! ship and any third party with the matching all-zero private key could
//! forge signatures for the plugin marketplace.
//!
//! Dev / debug / test builds may still compile without the env var; the
//! runtime falls back to "verification disabled" with a loud warning.

fn main() {
    println!("cargo:rerun-if-env-changed=AMIGO_REGISTRY_PUBKEY_HEX");
    println!("cargo:rerun-if-env-changed=PROFILE");

    let profile = std::env::var("PROFILE").unwrap_or_default();
    let pubkey_hex = std::env::var("AMIGO_REGISTRY_PUBKEY_HEX").ok();

    if profile == "release" {
        let Some(hex) = pubkey_hex else {
            panic!(
                "AMIGO_REGISTRY_PUBKEY_HEX must be set for release builds. \
                 Set it to the hex-encoded 32-byte Ed25519 public key of the \
                 plugin registry signer (64 lowercase hex chars)."
            );
        };
        if hex.len() != 64 {
            panic!(
                "AMIGO_REGISTRY_PUBKEY_HEX must be exactly 64 hex chars, got {}",
                hex.len()
            );
        }
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            panic!("AMIGO_REGISTRY_PUBKEY_HEX must be hex (0-9, a-f, A-F) only");
        }
        let zero = "0".repeat(64);
        if hex.eq_ignore_ascii_case(&zero) {
            panic!(
                "AMIGO_REGISTRY_PUBKEY_HEX is the all-zero placeholder; \
                 release builds must pin a real Ed25519 public key."
            );
        }
    }
}
