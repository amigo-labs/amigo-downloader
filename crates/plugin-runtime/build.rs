//! Build-time guards for the plugin-runtime crate.
//!
//! The plugin registry trusts whatever Ed25519 public key is baked into the
//! binary at compile time (see `registry::KRAKE_REGISTRY_PUBLIC_KEY`). For
//! release builds we refuse to compile without `KRAKE_REGISTRY_PUBKEY_HEX`
//! pointing at a real 32-byte key, otherwise an opaque "all zeros" key would
//! ship and any third party with the matching all-zero private key could
//! forge signatures for the plugin marketplace.
//!
//! Dev / debug / test builds may still compile without the env var; the
//! runtime falls back to "verification disabled" with a loud warning.

#[path = "src/guest_tools.rs"]
mod guest_tools;

include!("src/wasm_config.rs");

fn main() {
    build_guest();
    check_registry_pubkey();
}

/// Build the plugin guest (`crates/plugin-guest`, QuickJS as WebAssembly) and
/// precompile it for the target, so the plugin host loads it without a
/// compiler at run time. Writes `plugin_guest.cwasm` into `OUT_DIR`.
/// See docs/specs/plugin-wasm-guest.md.
fn build_guest() {
    use std::path::PathBuf;
    use std::process::Command;

    let env = |name: &str| std::env::var_os(name);
    let out_dir = PathBuf::from(env("OUT_DIR").expect("cargo sets OUT_DIR"));
    let manifest_dir =
        PathBuf::from(env("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let guest_dir = manifest_dir.join("../plugin-guest");
    for path in ["src", "Cargo.toml", "Cargo.lock"] {
        println!("cargo::rerun-if-changed={}", guest_dir.join(path).display());
    }
    println!("cargo::rerun-if-changed=src/wasm_config.rs");
    println!("cargo::rerun-if-changed=src/guest_tools.rs");
    println!("cargo::rerun-if-env-changed={}", guest_tools::CC_ENV);

    // Fail with a useful message before cargo and cc fail with theirs.
    let rustc = env("RUSTC").unwrap_or_else(|| "rustc".into());
    let target_libdir = Command::new(&rustc)
        .args([
            "--print",
            "target-libdir",
            "--target",
            guest_tools::GUEST_TARGET,
        ])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()));
    let search_path = env("PATH").unwrap_or_default();
    let cc_override = env(guest_tools::CC_ENV);
    if let Err(message) = guest_tools::check_guest_tools(
        &search_path,
        cc_override.as_deref(),
        target_libdir.as_deref(),
    ) {
        panic!("{message}");
    }
    if cc_override.is_none()
        && let Some(clang) = guest_tools::find_in_path("clang", &search_path)
    {
        let targets = Command::new(&clang).arg("--print-targets").output();
        if let Ok(out) = targets
            && out.status.success()
            && !String::from_utf8_lossy(&out.stdout).contains("wasm32")
        {
            panic!("{}", guest_tools::clang_without_wasm(&clang));
        }
    }

    // A nested cargo with its own target directory. The outer build's flags
    // and wrappers (clippy, cross-compilation settings) are for the host
    // target and must not leak into the Wasm build.
    let guest_target_dir = out_dir.join("guest-target");
    let mut cargo = Command::new(env("CARGO").unwrap_or_else(|| "cargo".into()));
    cargo
        .args(["build", "--release", "--locked", "--target"])
        .arg(guest_tools::GUEST_TARGET)
        .arg("--manifest-path")
        .arg(guest_dir.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&guest_target_dir);
    for name in [
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTFLAGS",
        "CARGO_TARGET_DIR",
        "CARGO_BUILD_TARGET",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "CLIPPY_ARGS",
        "CC",
        "CFLAGS",
        "AR",
    ] {
        cargo.env_remove(name);
    }
    let status = cargo
        .status()
        .unwrap_or_else(|e| panic!("could not run cargo for the plugin guest: {e}"));
    if !status.success() {
        panic!("building the plugin guest (crates/plugin-guest) failed: {status}");
    }
    let wasm = guest_target_dir
        .join(guest_tools::GUEST_TARGET)
        .join("release/krake_plugin_guest.wasm");
    let wasm = std::fs::read(&wasm)
        .unwrap_or_else(|e| panic!("cannot read the built guest {}: {e}", wasm.display()));

    // Precompile for the target this crate is built for (not the build
    // host), with the configuration `src/wasm.rs` loads it with.
    let target = std::env::var("TARGET").expect("cargo sets TARGET");
    let mut config = guest_engine_config();
    config
        .target(&target)
        .unwrap_or_else(|e| panic!("wasmtime cannot compile for {target}: {e}"));
    let engine = wasmtime::Engine::new(&config)
        .unwrap_or_else(|e| panic!("cannot create the wasmtime engine for {target}: {e}"));
    let precompiled = engine
        .precompile_module(&wasm)
        .unwrap_or_else(|e| panic!("cannot precompile the plugin guest for {target}: {e}"));
    std::fs::write(out_dir.join("plugin_guest.cwasm"), precompiled)
        .unwrap_or_else(|e| panic!("cannot write the precompiled guest: {e}"));
}

/// Release builds must pin the plugin registry's public key.
fn check_registry_pubkey() {
    println!("cargo::rerun-if-env-changed=KRAKE_REGISTRY_PUBKEY_HEX");
    println!("cargo::rerun-if-env-changed=PROFILE");

    let profile = std::env::var("PROFILE").unwrap_or_default();
    let pubkey_hex = std::env::var("KRAKE_REGISTRY_PUBKEY_HEX").ok();

    if profile == "release" {
        let Some(hex) = pubkey_hex else {
            panic!(
                "KRAKE_REGISTRY_PUBKEY_HEX must be set for release builds. \
                 Set it to the hex-encoded 32-byte Ed25519 public key of the \
                 plugin registry signer (64 lowercase hex chars)."
            );
        };
        if hex.len() != 64 {
            panic!(
                "KRAKE_REGISTRY_PUBKEY_HEX must be exactly 64 hex chars, got {}",
                hex.len()
            );
        }
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            panic!("KRAKE_REGISTRY_PUBKEY_HEX must be hex (0-9, a-f, A-F) only");
        }
        let zero = "0".repeat(64);
        if hex.eq_ignore_ascii_case(&zero) {
            panic!(
                "KRAKE_REGISTRY_PUBKEY_HEX is the all-zero placeholder; \
                 release builds must pin a real Ed25519 public key."
            );
        }
    }
}
