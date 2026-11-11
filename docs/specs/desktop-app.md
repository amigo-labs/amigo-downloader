---
status: todo
created: 2026-10-27
size: L
surfaces: [desktop, rest, web-ui]
---

# Feature: Desktop App (embedded server, tray, self-update)

## Summary

The Tauri app in `tauri/` has never shipped (no release tag exists), and as
built it cannot work: `tauri/src/main.rs` starts the `Coordinator` and
Click'n'Load but no HTTP server, while the web-ui talks to `/api/v1/...` and
`ws://<host>/api/v1/ws`. In the Tauri window those requests hit the bundled
asset protocol and fail. Plugins cannot run either, because
`krake-plugin-host` is not bundled, and config, downloads, plugins and cache
land in the process's working directory.

This spec makes the desktop app a working product and keeps it current:

1. **Embedded server.** The app runs the full `krake-server` stack on
   `127.0.0.1` and opens its window at that origin. The web-ui stays exactly
   as it is.
2. **Desktop session.** A per-launch random token is exchanged once for a
   session cookie. There is no setup wizard and no login, and no other local
   process can use the API without the token.
3. **Platform paths and plugin host.** Everything lives in the OS app
   directories, and `krake-plugin-host` ships as a Tauri sidecar.
4. **Tray, single instance, deep links.** Closing the window keeps downloads
   running in the tray. A second launch focuses the running app and hands
   over its `krake:`/`magnet:` link.
5. **Self-update**, modelled on amigo-pincel (`src-tauri/src/updater.rs`
   there):
   - a background check every `update.check_interval_hours` (default 24 h);
   - minisign verification against the organization key `3098504C1538B8F6`;
   - the new version takes effect at the next start, and Windows runs the
     verified installer when the app quits;
   - only the AppImage updates itself on Linux.

   The existing core-update banner in the web-ui drives it through a hook in
   `krake-server`.

   **Update choice** (revision 2026-11-11, alike in amigo-pincel and
   amigo-blinzel): on the first start a question offers "Install
   automatically (recommended)", "Notify me when an update is available" and
   "Never check for updates". The About section of the Settings page shows
   the version and every update option, where the choice can be changed.
   The **effective choice** is, in this order: the choice the user confirmed
   or selected in this run (even if storing it failed), else the stored one,
   else "install automatically" (also for copies that never get the
   question), because silent updates keep most copies current.
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
(AC-9 to AC-12). **P3** updater and release (AC-13 to AC-24, and the update
choice AC-25 to AC-35).

## Acceptance Criteria

### P1 — Embedded server

- [ ] AC-1: `krake_server::embedded::serve(opts)` starts the same routes and
  layers as the `krake-server` binary: API, WebSocket, updates, feedback,
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
  - `config.toml`, `krake.db`, `plugins/` and `cache/` live under the Tauri
    `app_data_dir`;
  - on first start `download_dir` defaults to
    `<user Downloads>/Krake` and `temp_dir` to `<download_dir>/.tmp`;
  - nothing is written to the working directory.
- [ ] AC-7: `krake-plugin-host` is bundled through `bundle.externalBin` for
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
- [ ] AC-11: A `krake://add?url=<encoded>` link, whether at launch or
  handed over by AC-10, adds `<url>` to the queue as `POST /downloads` does
  and shows the window. Malformed links are logged and ignored. The
  pre-rename scheme `amigo://` stays registered and is handled the same way
  (deprecated).
- [ ] AC-12: A `magnet:` link shows a notification saying torrents are not
  supported, and adds nothing.

### P3 — Self-update and release

- [ ] AC-13: While the effective update choice is "install automatically"
  or "notify", a background check of `latest.json` is due 10 s after start
  when no check is recorded or `update.check_interval_hours` have passed since
  `<app_data>/desktop.json` `last_update_check`, and again whenever the
  interval passes while the app runs. While it is "never check", no
  background check runs.
- [ ] AC-14: An update is downloaded and its minisign signature is verified
  against `plugins.updater.pubkey` (`3098504C1538B8F6`) before anything is
  installed. A wrong or missing signature installs nothing and is logged.
- [ ] AC-15: A verified update is installed by itself only while the update
  choice is "install automatically"; otherwise only after Install in the
  announcement (AC-28) or a manual apply. How it is installed depends on the
  platform:
  - **macOS** replaces the app bundle;
  - **Linux AppImage** replaces the AppImage, and the new version runs at the
    next start;
  - **Windows** keeps the installer in memory and runs it (passive mode) on
    tray **Quit**.

  An update never stops or restarts a running download; on Windows the
  installer runs only after tray **Quit** has ended the app.
- [ ] AC-16: The app never updates itself in debug builds or when it runs from
  a Linux `.deb`/`.rpm` install (no `APPIMAGE` env var).
- [ ] AC-17: In desktop mode `GET /api/v1/updates/check` reports the
  desktop update in `core`, plus `distribution: "tauri"`,
  `can_self_update` (from AC-16), `pending_restart`, `update_mode`
  (the effective choice: `"auto"`, `"notify"` or `"off"`) and
  `update_mode_chosen` (whether a choice is stored).
  `POST /api/v1/updates/core` runs check, download, verify and install (AC-15:
  replaced on macOS and the AppImage, held for Quit on Windows) through the
  hook, whatever the choice, and returns `{"status":"ready","version":"…"}`.
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
  - `krake-windows-x64-setup.exe` and `krake-windows-x64.msi`;
  - `krake-macos-x64.dmg` and `krake-macos-arm64.dmg`;
  - `krake-macos-x64.app.tar.gz` and `krake-macos-arm64.app.tar.gz`;
  - `krake-linux-x64.AppImage` and `krake-linux-x64.deb`;
  - each updater artifact also has its `.sig`.
- [ ] AC-23: The download section on amigo-labs.dev/krake/ links the AC-22
  assets per OS (`downloads` in `docs/site/project.yml`), instead of only the
  releases page.
- [ ] AC-24: A desktop release needs the organization secrets
  `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The
  installer job fails early with a clear message when either is missing.

### P3 — Update choice

- [ ] AC-25 (event): When the desktop app shows its window for the first time
  in a run and the copy can
  update itself (AC-16) and no update choice is stored, then the web-ui asks
  how Krake should handle updates, with the choices "Install automatically
  (recommended)", "Notify me when an update is available" and "Never check
  for updates", the first preselected, and a Confirm button. The question
  never shows in the browser against `krake-server` or Docker, in debug
  builds or in `.deb` / `.rpm` installs.
- [ ] AC-26 (event): When the user confirms the question, then the chosen
  option is stored and the question never shows again; when the user closes
  it without confirming (Escape, the close button, or closing the window to
  the tray — which the app itself counts as closing, even if the web-ui sent
  nothing), nothing is stored, "install automatically" applies for this run,
  and the question shows again at the next start.
- [ ] AC-27 (state): While the first-start question is neither confirmed nor
  closed in this run, no background update check starts; once it is, a check
  that came due meanwhile starts at once.
- [ ] AC-28 (event): When a background check finds a newer version while the
  update choice is "notify" and that version was not announced before, then
  nothing is downloaded and the web-ui shows "Krake <version> is available"
  with Install and Later; Install downloads, verifies and installs it like
  `POST /api/v1/updates/core` and then reads "Update ready — takes effect at
  the next start"; Later (also Escape or the close button), or an Install
  that succeeds, marks that version as announced, so later checks do not
  announce it again, while a still newer version is announced. An Install
  that fails shows the error and leaves the version unannounced, so the next
  due check announces it again.
- [ ] AC-29 (state): While the update choice is "never check", nothing is
  announced; the core-update banner's manual check and apply still work.
- [ ] AC-30 (state): While the Settings page's About section shows in the
  desktop app, it shows the running version (the release version, not a
  fixed string), the version that is downloaded and takes effect at the next
  start (if any), the three update choices with the stored one selected, and
  a Check for updates button, which works like the core-update banner's
  check and apply whatever the choice; selecting a choice stores it at once
  and takes effect without a restart: after "never check" no background check
  runs in this run, after "notify" the next due check announces instead of
  installing, after "install automatically" a due check starts at once. Where the copy cannot update itself it shows the
  reason instead of the choices ("update with your package manager" for
  `.deb` / `.rpm`). Outside the desktop app it shows the version only.
- [ ] AC-31 (event): In desktop mode `PUT /api/v1/updates/mode` with
  `{"mode":"auto"|"notify"|"off"}` makes the choice effective for this run,
  stores it and returns the state of AC-34 plus `saved: false` when storing
  failed; `{"dismissed":true}` records a close without a choice for this
  run; any other body is `400`. Outside desktop mode the route answers `404`.
- [ ] AC-34 (always): In desktop mode `GET /api/v1/updates/mode` answers
  without contacting the network, also offline and under "never check":
  `update_mode` (effective), `update_mode_chosen`, `question_pending` (no
  choice stored and the question not closed in this run), `can_self_update`,
  `pending_restart` and `announcement` (the version waiting to be announced,
  or `null`). The first-start question, the announcement and the About
  section read this, never `GET /updates/check`. Outside desktop mode `404`.
- [ ] AC-35 (event): In desktop mode `POST /api/v1/updates/announced` with
  `{"version":"x.y.z"}` marks exactly that version as announced (Later,
  Escape, close, or a successful Install) and stores it; a version other than
  the pending announcement, or any other body, is `400`. Showing the
  announcement marks nothing, so quitting with it open leaves it unannounced
  (AC-28). Outside desktop mode `404`.
- [ ] AC-32 (event, manual: needs a real Windows install): When the Windows
  setup runs silently (`/S`, as winget does) for a user who has no
  `desktop.json` yet, then "install automatically" is stored for that user
  and the first start shows no question, because a silent setup is a
  scripted or managed one whose users should not meet a question;
  interactive and passive (`/P`, the updater) installs store nothing, because
  the person who ran an interactive setup gets the question at the first
  start.
- [ ] AC-33 (always): Every text of the question, the announcement and the
  About section's update part exists in English and German.

## API Contract

### Rust Types

```rust
// crates/server/src/embedded.rs

/// Everything `serve` needs; the binary builds it from env/CLI, the desktop
/// app from Tauri's path resolver.
pub struct ServeOptions {
    pub config: krake_core::config::Config,
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
    pub coordinator: std::sync::Arc<krake_core::coordinator::Coordinator>,
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
    /// Update-choice state without network access (AC-34).
    fn mode_state(&self) -> DesktopModeState;
    /// Make a choice effective for this run and store it (AC-31).
    fn set_mode(&self, mode: UpdateMode) -> DesktopModeState;
    /// Record a close of the first-start question without a choice (AC-26).
    fn dismiss_question(&self);
    /// Mark `version` as announced; `Err` if it is not the pending one (AC-35).
    fn acknowledge(&self, version: &str) -> Result<(), String>;
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct DesktopModeState {
    pub update_mode: UpdateMode,
    pub update_mode_chosen: bool,
    pub question_pending: bool,
    pub can_self_update: bool,
    pub pending_restart: Option<String>,
    pub announcement: Option<String>,
    /// `false` after a failed write of `desktop.json` (only in PUT answers).
    pub saved: bool,
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
    /// The user's update choice; `None` until confirmed (AC-25, AC-26).
    pub update_mode: Option<UpdateMode>,
    /// Last version announced in "notify" (AC-28).
    pub announced_version: Option<String>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UpdateMode {
    Auto,
    Notify,
    Off,
}
```

### REST Endpoints

| Method | Path | Request Body | Response | Status Codes |
|--------|------|-------------|----------|-------------|
| GET | `/api/v1/desktop/session?token=<hex>` | — | `303 Location: /` + `Set-Cookie` | 303, 401 (wrong / used / no desktop mode), 429 (rate limit) |
| GET | `/api/v1/updates/check` | — | existing `UpdateCheck`; desktop mode: `core` from `DesktopUpdater::check`, plus `distribution: "tauri"`, `can_self_update`, `pending_restart: string \| null`, `update_mode: "auto" \| "notify" \| "off"` (effective), `update_mode_chosen: bool` | 200, 502 |
| POST | `/api/v1/updates/core` | — | desktop mode: `{"status":"ready","version":"1.2.3"}` | 200, 400 (not allowed, AC-16), 502 (check, download or signature failed) |
| GET | `/api/v1/updates/mode` | — | `DesktopModeState`, no network access (AC-34) | 200, 404 (no desktop mode) |
| PUT | `/api/v1/updates/mode` | `{"mode":"auto"\|"notify"\|"off"}` or `{"dismissed":true}` | `DesktopModeState` (with `saved`) | 200, 400 (other body), 404 (no desktop mode) |
| POST | `/api/v1/updates/announced` | `{"version":"x.y.z"}` | `DesktopModeState` | 200, 400 (not the pending announcement, other body), 404 (no desktop mode) |

`/api/v1/desktop/session` is mounted only when `ServeOptions::desktop` is
`Some`, and the token is compared in constant time. It is not behind
`require_auth`, and it is rate-limited like `/login`.

## Data Model Changes

- SQLite: none. The desktop session is a normal `sessions` row with the
  username `desktop`.
- New file `<app_data>/desktop.json` (`DesktopState`), written with
  temp-file + rename.
- Config: none. The desktop updater reuses `update.check_interval_hours`;
  whether it checks is the update choice in `desktop.json` (AC-13), not
  `update.auto_check`, which keeps governing plugin update checks.
- `desktop.json` also holds `update_mode` and `announced_version`. No
  migration: no desktop release has shipped, so no copy has a choice from an
  earlier version.

## UI Changes

- `web-ui/src/pages/Plugins.svelte`, core-update banner:
  - if `can_self_update` is false, show a hint instead of the button;
  - after apply, show "Update ready — takes effect at the next start";
  - if `pending_restart` is set, show the same text on load.
- New locale keys in `locales/en.json` and `locales/de.json`.
- No setup or login screen in desktop mode; this comes from the server
  (AC-4), with no web-ui change.
- First-start update question (a dialog over any page), the "notify"
  announcement (Install / Later), and the update part of the Settings page's
  About section (AC-25 to AC-30), all desktop mode only; outside it the About
  section shows the real version instead of the fixed `v0.1.0`.

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
  - `bundle.externalBin: ["binaries/krake-plugin-host"]`;
  - `bundle.createUpdaterArtifacts: true`;
  - the `"active"` key removed (Tauri v1 leftover);
  - no IPC capability for the remote origin.
- `crates/plugin-runtime/src/process_host.rs`: also look for the sidecar
  name with Tauri's target-triple suffix, if needed.
- `.github/workflows/release-build.yml`:
  - build `krake-plugin-host` per target into `tauri/binaries/`;
  - rename to the stable names;
  - write `latest.json` for Tauri 2;
  - signature key-ID guard;
  - check that the secrets are present.
- `docs/site/project.yml` (`downloads`): direct desktop links on amigo-labs.dev.
- `web-ui/src/pages/Plugins.svelte`, `locales/*.json`: banner states.
- `web-ui/src/pages/Settings.svelte` (About section: real version, update
  part in desktop mode), new components for the first-start question and the
  announcement, their strings in English and German (AC-25 to AC-30, AC-33).
- `tauri/windows/hooks.nsh`: new; the silent-install step of AC-32
  (`bundle.windows.nsis.installerHooks`).
- `docs/specs/tauri-workspace-integration.md`: point "Distribution
  detection" and the updater to this spec.

## Edge Cases

| Scenario | Expected Behavior |
|----------|------------------|
| Stored port taken by another program (e.g. a separate `krake-server` on 1516) | Bind `127.0.0.1:0`, store the new port, continue. `localStorage` settings reset once. |
| Session URL opened in an external browser with an old token | `401`; the token was single-use and per launch. |
| Another local process calls the API without cookie or token | `401` (AC-5). |
| Second launch with `krake://add?url=…` while the first instance is minimized to the tray | The first instance shows its window and queues the URL; the second exits with no second server. |
| `magnet:` link | Notification "torrents are not supported"; nothing queued. |
| Update found while downloads are running | Download and verify only; install takes effect at the next start (Windows: on Quit). Downloads continue. |
| Signature does not match the pubkey | Nothing installed; `POST /updates/core` → 502 with the reason; background check logs it and retries after the interval. |
| `latest.json` unreachable (offline) | Background check logs and records the check; banner shows no update; no error toast. |
| App runs from `.deb`/`.rpm` | `can_self_update: false`; banner shows "update with your package manager". |
| Windows: Quit while an update download is still in progress | Quit is not delayed. Nothing is installed; the next start checks again. |
| `krake-plugin-host` missing from a broken install | Plugin downloads fail with the existing "plugin host not found" error; built-in protocols keep working. |
| Release built with a repository secret that shadows the org key | AC-21 stops the upload, naming the file and both key IDs. |
| First start through a `krake://add` link | The link's download is queued; the question shows over the page. |
| Window closed to the tray while the question is open | Counts as closed without a choice (AC-26); downloads go on. |
| "notify" finds a version while the window is hidden in the tray | The announcement shows the next time the window opens. |
| Announcement open when the app quits | That version counts as not announced; the next due check announces it again. |
| `desktop.json` not writable when the user confirms | The choice applies for this run; the question shows again at the next start. |
| User switches from "never check" to "install automatically" in About | A check that is due starts without a restart. |
| Choice changes to "never check" while a check or download runs | That one completes and installs as before (Windows: on Quit); the new choice applies from the next check. |
| Banner apply, About check and a background check at the same time | Checks share one request to `latest.json`. An apply waiting on a check still downloads, verifies and installs afterwards; concurrent applies of the same version share one download. |
| A newer version comes out while the announcement of the previous one is open | The announcement names the newest version. |
| `desktop.json` holds a port but no `update_mode` | No choice stored: the question shows. |
| `desktop.json` not writable when the choice changes in About | The choice applies for this run; the section says it could not be saved. |
| `PUT /updates/mode {"dismissed":true}` fails or never arrives | Hiding the window or quitting counts as closing (AC-26); otherwise the question stays until the next start. |

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
- [ ] Integration: the `krake-server` binary still starts with the same routes
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
  - deep-link parsing: `krake://add?url=` (and `amigo://add?url=`) valid,
    malformed, and `magnet:`
    (AC-11, AC-12).
- [ ] Unit (`tauri`): `tauri.conf.json` has `createUpdaterArtifacts`,
  `externalBin` and the org pubkey (key ID `3098504C1538B8F6`) (AC-7, AC-14,
  AC-19).
- [ ] CI: a `cargo check -p krake-desktop --features tauri` job with the GTK
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
  - `krake://add` link;
  - update from version N to N+1 through the banner (AC-9 to AC-18).
- [ ] Manual: the website links resolve after the release (AC-22, AC-23).
- [ ] Unit (`tauri`): `UpdateMode` round trip in `desktop.json`, missing
  field → not chosen; `is_due` per mode; the announcement once per version
  (AC-13, AC-25 to AC-29).
- [ ] Unit (`crates/server`), `update_api` with a fake `DesktopUpdater`:
  `PUT /updates/mode` per mode, dismissed, bad body → 400, no desktop mode →
  404, a failed write → effective anyway with `saved: false` (AC-31);
  `GET /updates/mode` with an updater whose `check` panics → 200 (no network,
  AC-34); `POST /updates/announced` with the pending version, another
  version → 400 (AC-35); an apply that waits on a running check still
  installs (edge case).
- [ ] Web UI unit tests (`web-ui/test/`): question shown only for desktop
  mode with `can_self_update` and no choice; announcement once per version;
  About section states (AC-25, AC-28, AC-30).

## Out of Scope

- Code signing / notarization (SmartScreen and Gatekeeper warn on first
  launch, as for amigo-pincel).
- Rollback after a failed start — because Tauri's updater has no mechanism
  for it (as in amigo-pincel).
- Linux `--install` for the AppImage, and self-update of `.deb`/`.rpm`
  installs — because packages belong to the package manager and need root.
- A silent machine-wide install (`perMachine`, another account such as
  SYSTEM) — because the installer is per user (`installMode: currentUser`).
- Windows or Linux on ARM64.
- Tauri mobile.
- Torrent / magnet support (AC-12 only declines it).
- Moving `localStorage` settings into the server.
- Native file dialogs / Tauri IPC for the web-ui.
- An update question inside the Windows installer — because macOS has no
  installer and Linux has none either; one first-start question serves every
  platform alike (owner, 2026-11-11).
- An update choice for `krake-server`, Docker and the CLI — because they
  update through `krake update`, the banner or a new image; the question is
  about the desktop app replacing itself.
- Skipping a version for good, or postponing for a time span — because
  "notify" announces each version once and the banner's check offers it again
  at any time.

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-1 |      | open   |        |
| AC-2 |      | open   |        |
| AC-3 |      | open   |        |
| AC-4 |      | open   |        |
| AC-5 |      | open   |        |
| AC-6 |      | open   |        |
| AC-7 |      | open   |        |
| AC-8 |      | open   |        |
| AC-9 |      | open   |        |
| AC-10 |      | open   |        |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |
| AC-16 |      | open   |        |
| AC-17 |      | open   |        |
| AC-18 |      | open   |        |
| AC-19 |      | open   |        |
| AC-20 |      | open   |        |
| AC-21 |      | open   |        |
| AC-22 |      | open   |        |
| AC-23 |      | open   |        |
| AC-24 |      | open   |        |
| AC-25 |      | open   |        |
| AC-26 |      | open   |        |
| AC-27 |      | open   |        |
| AC-28 |      | open   |        |
| AC-29 |      | open   |        |
| AC-30 |      | open   |        |
| AC-31 |      | open   |        |
| AC-32 |      | open   |        |
| AC-33 |      | open   |        |
| AC-34 |      | open   |        |
| AC-35 |      | open   |        |

## Decisions

- 2026-11-11: Converted to the feature-spec template (front matter, Open
  questions, Evidence, Decisions); the earlier ACs keep their IDs and
  wording, the new ones carry their form.
- 2026-11-11: Update choice added (owner), alike in amigo-pincel
  (`desktop-app`) and amigo-blinzel (`updates`): the user picks on the first
  start — install automatically, notify, never check — and changes it in the
  Settings page's About section, which shows the version and every update
  option. Owner's answers: ask on the first start, not in the installer;
  three choices; nobody asked → automatic (closing without a choice, silent
  installs); "notify" announces each version once; closing the question
  without a choice asks again at the next start. AC-13, AC-15 and AC-17
  reworded; AC-25 to AC-33 new. The choice lives in `desktop.json`, not in
  the config TOML, because it belongs to the desktop app alone and the
  server's `update.auto_check` keeps governing plugin checks. New route
  `PUT /api/v1/updates/mode` (desktop mode only), listed for review.
- 2026-11-11, clarification pass: "effective choice" defined (the stored one,
  else "install automatically"), so copies without the question still check;
  AC-13 says when a check is due; AC-15 no longer claims an untestable
  "never interrupted"; AC-17 and the REST table agree, and
  `POST /updates/core` installs (or holds for Quit); a failed Install leaves
  the version unannounced, because the user asked for it; an update already
  running when the choice changes still completes, because stopping a
  half-done install is riskier; AC-32 applies per user and only without a
  `desktop.json`, because the hook cannot read JSON and the installer is per
  user. Reasons for the owner's defaults: automatic when nobody answered and
  preselected, because silent updates keep most copies current.
- 2026-11-11, review on #111 (Copilot): the choice made in this run wins over
  the stored one, also when storing it failed, because otherwise "applies for
  this run" would not hold; the web-ui reads the update-choice state from a
  network-free `GET /api/v1/updates/mode` (AC-34), because the question and
  About must work offline and under "never check"; a version is announced
  through `POST /api/v1/updates/announced` (AC-35), because the web-ui has no
  Tauri IPC and showing the announcement must not mark it; a waiting apply
  still installs after a shared check, because a check alone cannot satisfy
  an apply. Two more desktop-only routes, listed for review.
