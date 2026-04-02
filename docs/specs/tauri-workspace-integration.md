# Spec: Tauri Workspace Integration

**Date**: 2026-04-02
**Status**: Draft
**Scope**: Integrate the Tauri desktop app (`tauri/`) into the Cargo workspace with proper conditional compilation, fix broken updater config, and implement distribution detection.

---

## Problem Statement

The Tauri desktop app exists in the repository but is effectively disconnected from the workspace build. Four issues prevent it from being built, tested, or updated alongside the rest of the project:

1. **Tauri crate not in workspace members** — `Cargo.toml` workspace `members` array does not include `"tauri"`, so `cargo build --workspace` and `cargo test --workspace` skip it entirely. It cannot share `[workspace.dependencies]` reliably (even though it references them).
2. **Tauri dependencies commented out** — `tauri/Cargo.toml` has all Tauri-specific dependencies (`tauri`, `tauri-plugin-shell`, `tauri-plugin-notification`, `tauri-plugin-deep-link`, `tauri-plugin-updater`) commented out with "Uncomment when building with Tauri CLI". This means even if added to the workspace, a plain `cargo check` would compile only the `#[cfg(not(feature = "tauri"))]` stub.
3. **Empty updater public key** — `tauri/tauri.conf.json` has `"pubkey": ""` in the updater plugin config. Tauri's updater will refuse to verify signatures with an empty key, breaking auto-update entirely.
4. **Distribution detection not implemented** — `crates/core/src/updater.rs:59` has `// TODO: detect Tauri via runtime check` and always falls back to `Distribution::Server`. The core update system never knows it is running inside Tauri, so it may attempt self-replace instead of deferring to `tauri-plugin-updater`.

---

## Design

### 1. Add `tauri/` to workspace members (conditionally excluded from default build)

Add `"tauri"` to the workspace `members` array so it participates in dependency resolution and lockfile generation. Use a Cargo feature flag on the tauri crate to gate the heavy Tauri dependencies, so `cargo check --workspace` still succeeds without Tauri system libraries installed.

**Workspace root `Cargo.toml`:**
```toml
members = [
    "crates/core",
    "crates/extractors",
    "crates/plugin-runtime",
    "crates/server",
    "crates/cli",
    "tauri",
]
```

Optionally, use `[workspace.metadata]` or `default-members` to exclude tauri from the default build target:
```toml
default-members = [
    "crates/core",
    "crates/extractors",
    "crates/plugin-runtime",
    "crates/server",
    "crates/cli",
]
```

This way `cargo build` builds only the default members, while `cargo build -p amigo-desktop` or `cargo tauri build` explicitly builds the Tauri crate.

### 2. Uncomment Tauri dependencies behind a feature flag

Replace the commented-out dependencies with feature-gated optional dependencies:

**`tauri/Cargo.toml`:**
```toml
[features]
default = []
tauri = ["dep:tauri", "dep:tauri-plugin-shell", "dep:tauri-plugin-notification", "dep:tauri-plugin-deep-link", "dep:tauri-plugin-updater"]

[dependencies]
# ... existing workspace deps ...

# Tauri dependencies (activated by "tauri" feature, which cargo tauri build enables)
tauri = { version = "2", features = ["tray-icon", "protocol-asset"], optional = true }
tauri-plugin-shell = { version = "2", optional = true }
tauri-plugin-notification = { version = "2", optional = true }
tauri-plugin-deep-link = { version = "2", optional = true }
tauri-plugin-updater = { version = "2", optional = true }
```

The existing `#[cfg(feature = "tauri")]` guards in `tauri/src/main.rs` already handle conditional compilation correctly. The `#[cfg(not(feature = "tauri"))]` fallback main prints build instructions.

### 3. Fix updater public key

The empty `"pubkey": ""` in `tauri/tauri.conf.json` must be replaced with either:
- A real Ed25519 public key generated via `tauri signer generate`
- Or the updater plugin section should be removed/disabled until a key is generated

**Implementation approach:**
- Add a `scripts/generate-tauri-signing-key.sh` script that wraps `tauri signer generate -w ~/.tauri/amigo-downloader.key` and outputs the public key
- Document the key generation in the Tauri build instructions
- Set a placeholder comment in `tauri.conf.json` explaining the key must be generated before release builds
- In CI, the signing key should come from a secret; local dev builds can disable the updater

For development, disable the updater plugin to avoid the empty-key error:
```json
"updater": {
    "active": false,
    "endpoints": [
        "https://github.com/amigo-labs/amigo-downloader/releases/latest/download/latest.json"
    ],
    "pubkey": "REPLACE_WITH_REAL_KEY_BEFORE_RELEASE"
}
```

### 4. Implement Tauri distribution detection

In `crates/core/src/updater.rs`, the `detect_distribution()` function needs to identify when running inside Tauri. Since the core crate cannot depend on Tauri, use an environment variable or compile-time feature flag:

**Option A — Environment variable (preferred):**
The Tauri `main.rs` sets `AMIGO_TAURI=1` at startup (before any core initialization), mirroring the existing `AMIGO_DOCKER` pattern:

```rust
pub fn detect_distribution() -> Distribution {
    if std::env::var("AMIGO_DOCKER").is_ok() {
        return Distribution::Docker;
    }
    if std::env::var("AMIGO_TAURI").is_ok() {
        return Distribution::Tauri;
    }
    // Check binary name as a heuristic
    if let Some(name) = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
    {
        if name.contains("amigo-desktop") || name.contains("amigo-downloader") {
            // Could be Tauri, but without env var we can't be sure
        }
    }
    Distribution::Server
}
```

In `tauri/src/main.rs`, at the top of the Tauri main function:
```rust
std::env::set_var("AMIGO_TAURI", "1");
```

**Behavior when `Distribution::Tauri` is detected:**
- `can_self_update` should return `false` for the core updater (Tauri has its own updater mechanism via `tauri-plugin-updater`)
- The `/api/v1/status` endpoint should report `distribution: "tauri"` so the web-ui can adapt (e.g., hide self-update prompts, show Tauri-native update UI)

---

## Acceptance Criteria

### AC1: Workspace membership
- [ ] `tauri` appears in `[workspace.members]` in root `Cargo.toml`
- [ ] `cargo metadata --no-deps` includes `amigo-desktop` in the package list
- [ ] `cargo check --workspace` succeeds without Tauri system libraries installed (the `tauri` feature is not enabled by default)
- [ ] `cargo build -p amigo-desktop` compiles the stub binary (prints instructions) without errors

### AC2: Feature-gated Tauri dependencies
- [ ] `tauri/Cargo.toml` has no commented-out dependencies
- [ ] All Tauri crate dependencies are declared as `optional = true` and gated behind a `tauri` feature
- [ ] `#[cfg(feature = "tauri")]` blocks in `main.rs` compile when the feature is enabled
- [ ] `cargo clippy -p amigo-desktop` passes (without `tauri` feature, checking the stub path)

### AC3: Updater public key
- [ ] `tauri/tauri.conf.json` does not contain `"pubkey": ""`
- [ ] Either a valid placeholder with documentation comment exists, or the updater is marked inactive for dev builds
- [ ] A script or documentation describes how to generate and configure the signing key

### AC4: Distribution detection
- [ ] `detect_distribution()` returns `Distribution::Tauri` when `AMIGO_TAURI` env var is set
- [ ] The Tauri `main.rs` sets `AMIGO_TAURI=1` before initializing the core
- [ ] When distribution is `Tauri`, `can_self_update` is `false` (Tauri manages its own updates)
- [ ] Unit test: `detect_distribution()` returns correct variant for each env var combination

### AC5: No regressions
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` passes
- [ ] Existing CI (server, CLI, core, extractors, plugin-runtime) is unaffected
- [ ] Docker build is unaffected

---

## Implementation Notes

### Workspace `default-members`
Using `default-members` is important because Tauri v2 requires system libraries (GTK, WebKit on Linux) that may not be present on all developer machines or CI runners. Without `default-members`, adding `tauri` to `members` would cause `cargo build` to fail on machines without those libraries — even though the `tauri` feature is not active — because Cargo still resolves and downloads all workspace members.

With `default-members`, plain `cargo build` only builds the server/CLI crates. The Tauri crate is only built when explicitly requested via `cargo build -p amigo-desktop` or `cargo tauri build`.

### Feature flag naming
The feature is named `tauri` to match the existing `#[cfg(feature = "tauri")]` guards already present in `tauri/src/main.rs`. No code changes needed in the cfg attributes.

### Tauri CLI interaction
`cargo tauri build` and `cargo tauri dev` automatically pass the correct features. Verify that Tauri CLI v2 enables the `tauri` feature by default, or configure it in `tauri.conf.json` under `build.features`.

### CI considerations
- Add a separate CI job for Tauri builds that installs system dependencies (e.g., `libwebkit2gtk-4.1-dev`, `libgtk-3-dev` on Ubuntu)
- The standard CI jobs should use `default-members` and never trigger Tauri compilation
- Signing key for updater should be stored as a CI secret (`TAURI_SIGNING_PRIVATE_KEY`)

---

## Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Adding `tauri` to workspace breaks `cargo build` on machines without GTK/WebKit | High — blocks all developers | Use `default-members` to exclude tauri from default build |
| Tauri dependency versions conflict with workspace deps | Medium — version resolution failures | Pin Tauri deps explicitly, test lockfile resolution |
| `cargo tauri build` may not respect workspace feature flags | Medium — build failure | Test with Tauri CLI v2, add `build.features = ["tauri"]` to `tauri.conf.json` if needed |
| Signing key management adds operational complexity | Low — only affects release process | Document key generation, store private key in CI secrets only |
| `AMIGO_TAURI` env var could be set accidentally by users | Low — wrong update path selected | Use a specific value check (`AMIGO_TAURI=1`) rather than just presence, document the variable |

---

## Out of Scope

- Actual Tauri app functionality improvements (tray icon, deep link handling)
- Mobile Tauri build configuration
- Tauri plugin marketplace integration
- CI pipeline creation (referenced in notes but not implemented in this spec)
