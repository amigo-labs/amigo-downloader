# Feature: Desktop App (embedded server, tray, self-update)

> status: draft

## Summary

The Tauri app in `tauri/` has never shipped (no release tag exists), and as
built it cannot work: `tauri/src/main.rs` starts the `Coordinator` and
Click'n'Load but no HTTP server, while the web-ui talks to `/api/v1/...` and
`ws://<host>/api/v1/ws`. In the Tauri window those requests hit the bundled
asset protocol and fail. Plugins cannot run either, because
`amigo-plugin-host` is not bundled, and config, downloads, plugins and cache
land in the process's working directory.

This spec makes the desktop app a working product and keeps it current:

1. **Embedded server.** The app runs the full `amigo-server` stack on
   `127.0.0.1` and opens its window at that origin. The web-ui stays exactly
   as it is.
2. **Desktop session.** A per-launch random token is exchanged once for a
   session cookie. There is no setup wizard and no login, and no other local
   process can use the API without the token.
3. **Platform paths and plugin host.** Everything lives in the OS app
   directories, and `amigo-plugin-host` ships as a Tauri sidecar.
4. **Tray, single instance, deep links.** Closing the window keeps downloads
   running in the tray. A second launch focuses the running app and hands
   over its `amigo:`/`magnet:` link.
5. **Self-update**, modelled on amigo-pincel (`src-tauri/src/updater.rs`
   there):
   - a background check every `update.check_interval_hours` (default 24 h);
   - minisign verification against the organization key `3098504C1538B8F6`;
   - the new version takes effect at the next start, and Windows runs the
     verified installer when the app quits;
   - only the AppImage updates itself on Linux.

   The existing core-update banner in the web-ui drives it through a hook in
   `amigo-server`.
6. **Release plumbing:**
   - `createUpdaterArtifacts`;
   - a Tauri-2 `latest.json`;
   - a check before publishing that every signature carries the key ID of
     `plugins.updater.pubkey`;
   - stable asset names linked from the website.

Key decision: the desktop app embeds the same server the Docker image runs,
instead of re-implementing the API over Tauri IPC. One API, one web-ui, one
code path to test. The webview gets no Tauri IPC capabilities, so the remote
origin cannot call native commands.

Delivered in three PRs, in this order: **P1** embedded server, session, paths
and sidecar (AC-1 to AC-8). **P2** tray, single instance and deep links
(AC-9 to AC-12). **P3** updater and release (AC-13 to AC-24).

## Acceptance Criteria

### P1 — Embedded server

- [ ] AC-1: `amigo_server::embedded::serve(opts)` starts the same routes and
  layers as the `amigo-server` binary: API, WebSocket, updates, feedback,
  NZBGet, static web-ui, CORS and security headers. It does so on a
  `TcpListener` the caller has already bound. The binary's `main` calls the
  same function, so the two cannot drift.
- [ ] AC-2: On start the desktop app binds `127.0.0.1` on the port stored in
  `<app_data>/desktop.json`. If that port is taken, it binds `127.0.0.1:0`
  and stores the new port. The window opens at `http://127.0.0.1:<port>/`.
- [ ] AC-3: The app generates a 32-byte random token per launch and first
  loads `http://127.0.0.1:<port>/api/v1/desktop/session?token=<hex>`.
  - A correct token gets a `303` to `/`, with a `Set-Cookie` session cookie
    (HttpOnly, SameSite=Strict) for the principal `desktop`.
  - The token is single-use: a second request with it gets `401`.
- [ ] AC-4: In desktop mode, `GET /api/v1/setup/status` reports
  `needs_setup: false`, and the web-ui opens straight on the Downloads page
  with no wizard or login screen.
- [ ] AC-5: API requests with no cookie, token or `api_token` get `401` in
  desktop mode, as in server mode. Desktop mode adds no loopback trust.
- [ ] AC-6: Desktop paths:
  - `config.toml`, `amigo.db`, `plugins/` and `cache/` live under the Tauri
    `app_data_dir`;
  - on first start `download_dir` defaults to
    `<user Downloads>/amigo-downloader` and `temp_dir` to `<download_dir>/.tmp`;
  - nothing is written to the working directory.
- [ ] AC-7: `amigo-plugin-host` is bundled through `bundle.externalBin` for
  every desktop target. `process_host` finds it in the installed app, and
  a plugin URL (generic-http) resolves in a release build.
- [ ] AC-8: Click'n'Load starts exactly once: by the embedded server, and not
  also by `tauri/src/main.rs`.

### P2 — Tray, single instance, deep links

- [ ] AC-9: Closing the window hides it; downloads keep running. The tray
  menu offers **Open**, **Pause all** / **Resume all** and **Quit**. A left
  click opens the window. Only **Quit** ends the process.
- [ ] AC-10: A second launch does not start a second server. It focuses the
  running window, restoring it if it is minimized or hidden, and hands over
  its command-line arguments (tauri-plugin-single-instance).
- [ ] AC-11: An `amigo://add?url=<encoded>` link, whether at launch or
  handed over by AC-10, adds `<url>` to the queue as `POST /downloads` does
  and shows the window. Malformed links are logged and ignored.
- [ ] AC-12: A `magnet:` link shows a notification saying torrents are not
  supported, and adds nothing.

### P3 — Self-update and release

- [ ] AC-13: With `update.auto_check = true`, a background thread checks
  `latest.json` 10 s after start, and then whenever
  `update.check_interval_hours` have passed since `<app_data>/desktop.json`
  `last_update_check`.
- [ ] AC-14: An update is downloaded and its minisign signature is verified
  against `plugins.updater.pubkey` (`3098504C1538B8F6`) before anything is
  installed. A wrong or missing signature installs nothing and is logged.
- [ ] AC-15: How a verified update is installed depends on the platform:
  - **macOS** replaces the app bundle;
  - **Linux AppImage** replaces the AppImage, and the new version runs at the
    next start;
  - **Windows** keeps the installer in memory and runs it (passive mode) on
    tray **Quit**.

  Running downloads are never interrupted by an update.
- [ ] AC-16: The app never updates itself in debug builds or when it runs from
  a Linux `.deb`/`.rpm` install (no `APPIMAGE` env var).
- [ ] AC-17: In desktop mode `GET /api/v1/updates/check` reports the
  desktop update in `core`, plus `distribution: "tauri"`,
  `can_self_update` (from AC-16) and `pending_restart`.
  `POST /api/v1/updates/core` runs check, download and verify through the
  hook, and returns `{"status":"ready","version":"…"}`.
- [ ] AC-18: The web-ui core-update banner works in desktop mode with no new
  page. After a successful apply it reads "Update ready — takes effect at the
  next start". When `can_self_update` is false it shows a hint instead of the
  button (deb/rpm: "update with your package manager").
- [ ] AC-19: `tauri.conf.json` sets `bundle.createUpdaterArtifacts: true`.
  The release build produces `.sig` files for the Windows setup, the MSI, the
  macOS `.app.tar.gz` (x64 and arm64) and the Linux AppImage.
- [ ] AC-20: `release-build.yml` writes a Tauri-2 `latest.json`:
  - `version` without the `v`;
  - platforms `windows-x86_64` (+`-nsis`, `-msi`), `darwin-x86_64`,
    `darwin-aarch64` and `linux-x86_64` (+`-appimage`).

  It fails if any of those signatures is missing, and it never writes an
  empty `signature`.
- [ ] AC-21: Before uploading, the upload job checks that every Tauri `.sig`
  carries the key ID of `plugins.updater.pubkey`, and fails with the file name
  and both IDs otherwise. A rotation's bridge release overrides this with the
  repository variable `UPDATER_SIGNING_KEY_ID`, as in amigo-pincel.
- [ ] AC-22: Desktop release assets use stable names, so
  `releases/latest/download/<name>` always resolves:
  - `amigo-downloader-windows-x64-setup.exe` and `amigo-downloader-windows-x64.msi`;
  - `amigo-downloader-macos-x64.dmg` and `amigo-downloader-macos-arm64.dmg`;
  - `amigo-downloader-macos-x64.app.tar.gz` and `amigo-downloader-macos-arm64.app.tar.gz`;
  - `amigo-downloader-linux-x64.AppImage` and `amigo-downloader-linux-x64.deb`;
  - each updater artifact also has its `.sig`.
- [ ] AC-23: The site's "Desktop" install tab
  (`site/src/components/sections/InstallTabs.svelte`) links the AC-22 assets
  per OS, instead of only the releases page.
- [ ] AC-24: A desktop release needs the organization secrets
  `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The
  installer job fails early with a clear message when either is missing.

## API Contract

### Rust Types

```rust
// crates/server/src/embedded.rs

/// Everything `serve` needs; the binary builds it from env/CLI, the desktop
/// app from Tauri's path resolver.
pub struct ServeOptions {
    pub config: amigo_core::config::Config,
    /// Where setup / PUT /config persist the config.
    pub config_path: std::path::PathBuf,
    pub db_path: std::path::PathBuf,
    pub plugins_dir: std::path::PathBuf,
    pub cache_dir: std::path::PathBuf,
    /// Already bound; its local address decides loopback handling.
    pub listener: tokio::net::TcpListener,
    pub start_clicknload: bool,
    /// `Some` in the desktop app: enables /api/v1/desktop/session and the
    /// desktop update hook.
    pub desktop: Option<DesktopOptions>,
}

pub struct DesktopOptions {
    /// Per-launch token, 32 random bytes, lowercase hex. Single use.
    pub session_token: String,
    pub updater: std::sync::Arc<dyn DesktopUpdater>,
}

/// Handle returned by `serve`; dropping it does not stop the server.
pub struct ServerHandle {
    pub local_addr: std::net::SocketAddr,
    pub coordinator: std::sync::Arc<amigo_core::coordinator::Coordinator>,
    pub shutdown: tokio::sync::oneshot::Sender<()>,
}

pub async fn serve(opts: ServeOptions) -> anyhow::Result<ServerHandle>;

// crates/server/src/update_api.rs

/// Implemented by the Tauri app (tauri/src/updater.rs) on top of
/// tauri-plugin-updater; the server only calls it.
#[async_trait::async_trait]
pub trait DesktopUpdater: Send + Sync {
    /// Whether this copy may update itself (AC-16).
    fn can_self_update(&self) -> bool;
    /// Version downloaded and verified in this session, if any.
    fn pending_restart(&self) -> Option<String>;
    /// Check only; `None` when up to date.
    async fn check(&self) -> Result<Option<DesktopUpdateInfo>, String>;
    /// Check, download, verify and install / hold (AC-14, AC-15).
    async fn apply(&self) -> Result<String, String>; // returns the version
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DesktopUpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

// tauri/src/state.rs — <app_data>/desktop.json
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct DesktopState {
    /// Last port the embedded server used (AC-2).
    pub port: Option<u16>,
    /// Unix seconds of the last background update check (AC-13).
    pub last_update_check: u64,
}
```

### REST Endpoints

| Method | Path | Request Body | Response | Status Codes |
|--------|------|-------------|----------|-------------|
| GET | `/api/v1/desktop/session?token=<hex>` | — | `303 Location: /` + `Set-Cookie` | 303, 401 (wrong / used / no desktop mode), 429 (rate limit) |
| GET | `/api/v1/updates/check` | — | existing `UpdateCheck`; desktop mode: `core` from `DesktopUpdater::check`, plus `distribution: "tauri"`, `can_self_update`, `pending_restart: string \| null` | 200, 502 |
| POST | `/api/v1/updates/core` | — | desktop mode: `{"status":"ready","version":"1.2.3"}` | 200, 400 (not allowed, AC-16), 502 (check, download or signature failed) |

`/api/v1/desktop/session` is mounted only when `ServeOptions::desktop` is
`Some`, and the token is compared in constant time. It is not behind
`require_auth`, and it is rate-limited like `/login`.

## Data Model Changes

- SQLite: none. The desktop session is a normal `sessions` row with the
  username `desktop`.
- New file `<app_data>/desktop.json` (`DesktopState`), written with
  temp-file + rename.
- Config: none. The desktop updater reuses `update.auto_check` and
  `update.check_interval_hours`.

## UI Changes

- `web-ui/src/pages/Plugins.svelte`, core-update banner:
  - if `can_self_update` is false, show a hint instead of the button;
  - after apply, show "Update ready — takes effect at the next start";
  - if `pending_restart` is set, show the same text on load.
- New locale keys in `locales/en.json` and `locales/de.json`.
- No setup or login screen in desktop mode; this comes from the server
  (AC-4), with no web-ui change.

## Affected Files

- `crates/server/src/embedded.rs`: new; `ServeOptions`, `serve`, and the
  wiring moved out of `main.rs`.
- `crates/server/src/main.rs`: builds `ServeOptions` from env/config and
  calls `serve`.
- `crates/server/src/desktop_session.rs`: new; the token exchange endpoint.
- `crates/server/src/auth.rs`, `setup.rs`: desktop mode reports setup as
  complete. Loopback detection takes the listener's address.
- `crates/server/src/update_api.rs`: `DesktopUpdater` trait and the
  desktop branch.
- `crates/core/src/config.rs`: remove the false "On loopback we skip setup
  entirely" comment.
- `tauri/src/main.rs`: bind, `serve`, open the window at the session URL,
  tray, single instance, deep links. Drop its own Coordinator, runtime and
  Click'n'Load.
- `tauri/src/updater.rs`: new; port of amigo-pincel's updater implementing
  `DesktopUpdater`.
- `tauri/src/state.rs`: new; `DesktopState` load/save.
- `tauri/Cargo.toml`: add `tauri-plugin-single-instance` (new dependency) and
  `async-trait` if the trait needs it.
- `tauri/tauri.conf.json`:
  - no static window (it is created at runtime with an external URL);
  - `bundle.externalBin: ["binaries/amigo-plugin-host"]`;
  - `bundle.createUpdaterArtifacts: true`;
  - the `"active"` key removed (Tauri v1 leftover);
  - no IPC capability for the remote origin.
- `crates/plugin-runtime/src/process_host.rs`: also look for the sidecar
  name with Tauri's target-triple suffix, if needed.
- `.github/workflows/release-build.yml`:
  - build `amigo-plugin-host` per target into `tauri/binaries/`;
  - rename to the stable names;
  - write `latest.json` for Tauri 2;
  - signature key-ID guard;
  - check that the secrets are present.
- `site/src/components/sections/InstallTabs.svelte`: direct desktop links.
- `web-ui/src/pages/Plugins.svelte`, `locales/*.json`: banner states.
- `docs/specs/tauri-workspace-integration.md`: point "Distribution
  detection" and the updater to this spec.

## Edge Cases

| Scenario | Expected Behavior |
|----------|------------------|
| Stored port taken by another program (e.g. a separate `amigo-server` on 1516) | Bind `127.0.0.1:0`, store the new port, continue. `localStorage` settings reset once. |
| Session URL opened in an external browser with an old token | `401`; the token was single-use and per launch. |
| Another local process calls the API without cookie or token | `401` (AC-5). |
| Second launch with `amigo://add?url=…` while the first instance is minimized to the tray | The first instance shows its window and queues the URL; the second exits with no second server. |
| `magnet:` link | Notification "torrents are not supported"; nothing queued. |
| Update found while downloads are running | Download and verify only; install takes effect at the next start (Windows: on Quit). Downloads continue. |
| Signature does not match the pubkey | Nothing installed; `POST /updates/core` → 502 with the reason; background check logs it and retries after the interval. |
| `latest.json` unreachable (offline) | Background check logs and records the check; banner shows no update; no error toast. |
| App runs from `.deb`/`.rpm` | `can_self_update: false`; banner shows "update with your package manager". |
| Windows: Quit while an update download is still in progress | Quit is not delayed. Nothing is installed; the next start checks again. |
| `amigo-plugin-host` missing from a broken install | Plugin downloads fail with the existing "plugin host not found" error; built-in protocols keep working. |
| Release built with a repository secret that shadows the org key | AC-21 stops the upload, naming the file and both key IDs. |

## Test Plan

- [ ] Unit (`crates/server`):
  - desktop session: the right token → 303 + cookie;
  - a reused token → 401;
  - the wrong token → 401;
  - no desktop mode → the route does not exist (AC-3).
- [ ] Unit (`crates/server`): `setup/status` → `needs_setup: false` in desktop
  mode (AC-4).
- [ ] Integration (`crates/server/tests`):
  - `serve` on `127.0.0.1:0`: `/api/v1/status` without auth → 401 (AC-5);
  - with the desktop cookie → 200;
  - the static `index.html` is served (AC-1).
- [ ] Integration: the `amigo-server` binary still starts with the same routes
  through `serve`: an existing integration test or a new smoke test (AC-1).
- [ ] Unit (`crates/server`), `update_api` with a fake `DesktopUpdater`:
  - check (update / none);
  - apply → ready;
  - not allowed → 400;
  - failure → 502 (AC-17).
- [ ] Unit (`tauri`):
  - `DesktopState` round trip and a damaged file → defaults;
  - port fallback when the stored port is taken (AC-2);
  - `is_due` interval and clock-went-back cases (AC-13);
  - `can_self_update` in debug, with `APPIMAGE` and without it (AC-16);
  - deep-link parsing: `amigo://add?url=` valid, malformed, and `magnet:`
    (AC-11, AC-12).
- [ ] Unit (`tauri`): `tauri.conf.json` has `createUpdaterArtifacts`,
  `externalBin` and the org pubkey (key ID `3098504C1538B8F6`) (AC-7, AC-14,
  AC-19).
- [ ] CI: a `cargo check -p amigo-desktop --features tauri` job with the GTK
  and WebKit libraries, so the Tauri code is compiled on every PR (today
  nothing compiles it).
- [ ] Workflow:
  - the `latest.json` generator and the key-ID guard are scripts under
    `scripts/` with a smoke test in `ci.yml`, like the existing
    `sign-update-archives.sh` smoke test;
  - fixtures: four real `.sig` files from the org key → pass;
  - a `.sig` from another key → fail;
  - a missing signature → fail (AC-20, AC-21).
- [ ] Manual (first release), on Windows, macOS and Linux:
  - install, first start opens without login;
  - download through a plugin;
  - close to tray, reopen;
  - second launch focuses;
  - `amigo://add` link;
  - update from version N to N+1 through the banner (AC-9 to AC-18).
- [ ] Manual: the website links resolve after the release (AC-22, AC-23).

## Out of Scope

- Code signing / notarization (SmartScreen and Gatekeeper warn on first
  launch, as for amigo-pincel).
- Rollback after a failed start.
- Linux `--install` for the AppImage, and self-update of `.deb`/`.rpm`
  installs.
- Windows or Linux on ARM64.
- Tauri mobile.
- Torrent / magnet support (AC-12 only declines it).
- Moving `localStorage` settings into the server.
- Native file dialogs / Tauri IPC for the web-ui.
