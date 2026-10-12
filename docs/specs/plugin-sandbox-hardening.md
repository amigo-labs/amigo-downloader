# Plugin Sandbox Hardening

**Date**: 2026-10-02
**Status**: Implemented (#78, #80–#84, #79 process split), design note (#85), open (#79 OS sandbox)
**Scope**: `crates/plugin-runtime`, server plugin endpoints, Web UI / CLI install flow, release workflow
**Tracking**: #77

---

## Summary

The plugin sandbox's documented limits (30 s, 64 MB, 20 requests, 1 MB storage)
bounded a *cooperating* interpreter but were not a security boundary: some
entry points ran without a deadline, the registry signature check could be
bypassed, and any plugin could reach any host. This spec covers the in-process
hardening and the process split (#79, SH-7a). The OS sandbox around the
child process (SH-7b) is the remaining step that changes the trust model; its
design decisions are recorded at the end.

---

## SH-1. Registry signature verification is load-bearing (#80)

**Acceptance criteria**

- [x] No construction path can set the trusted key to the all-zero placeholder.
  `RegistryConfig::index_verifier` is private; the server uses
  `RegistryConfig::for_index`, derived from `Default`;
  `with_index_verifier` rejects low-order keys.
  Test: `server_construction_path_never_trusts_the_zero_key`,
  `explicit_low_order_keys_are_rejected`.
- [x] `verify_ed25519` uses `verify_strict` and rejects low-order public keys.
  Test: `verify_ed25519_rejects_forgery_under_low_order_key`.
- [x] The cache stores the exact signed bytes plus `index.json.sig`, and every
  cache read re-verifies them. If verification fails, the cache counts as a
  miss and the index is refetched.
  Test: `cached_index_is_reverified_and_tampering_forces_a_refetch`.
- [x] `release-build.yml` passes `AMIGO_REGISTRY_PUBKEY_HEX` and
  `AMIGO_UPDATE_PUBKEY_HEX` (repository *variables* — public keys) to native,
  cross and Docker builds. It fails early if they are unset. CI runs
  `cargo check --release` to exercise the `build.rs` guards.
- [x] `RegistryPlugin::id` must match `^[a-z0-9][a-z0-9-]{0,63}$` before it is
  used as a path component.
  Test: `plugin_ids_are_validated_as_path_components`,
  `download_rejects_traversal_ids_and_oversized_artifacts`.
- [x] Size caps on downloads: the index is limited to 4 MiB, the signature to
  1 KiB and the artifact to 2 MiB. Only UTF-8 `.ts`/`.js` source artifacts
  are accepted.

Key rotation stays fail-closed: a new key ships in a new release, and old
clients lose marketplace access until they update. This is intentional. A
second, overlapping key is a possible follow-up.

## SH-2. Every JS entry point runs under a deadline (#81)

- [x] `PluginContext::with_deadline` is the only path into the QuickJS context.
  It covers module evaluation, export reads (getters included), `resolve`,
  `postProcess`, spec runs and host-API registration.
- [x] Module evaluation has its own budget (`SandboxLimits::max_load_secs`,
  default 5 s). Test: `infinite_loop_in_module_body_fails_to_load_within_budget`,
  `looping_getter_on_manifest_export_is_interrupted`.
- [x] A catastrophically backtracking native `RegExp` is interrupted. quickjs-ng
  0.8 (rquickjs 0.9) did not poll the interrupt handler inside `libregexp`, so
  rquickjs is bumped to 0.14 (quickjs-ng 0.16.2), which does
  (`lre_check_timeout`). Test: `catastrophic_native_regexp_is_interrupted`.

## SH-3. Plugin execution does not block the async runtime (#82)

- [x] Each plugin gets its **own** `rquickjs::Runtime`. This makes the memory
  limit and the deadline per plugin, and lets plugins run concurrently. rquickjs
  serializes all contexts of one runtime behind a single lock.
- [x] Execution runs on `spawn_blocking`. The plugin list is an `RwLock` that is
  held only to look a plugin up. A per-plugin exec lock serializes calls to the
  same plugin, and that lock moves into the blocking task.
- [x] HTTP request counters are per plugin (`HostApi::scoped`).
- Tests: `slow_plugin_does_not_block_other_plugins_or_metadata` (list, match
  and an unrelated resolve finish in under 2 s while a 3 s plugin spins) and
  `concurrent_plugins_each_keep_their_own_deadline`.

## SH-4. Per-plugin domain allowlist (#78)

- [x] Plugins can declare `permissions.domains`. Entries are exact hosts or
  `*.example.com`. Schemes, paths, ports and a bare `*` are rejected at load.
- [x] The compiled allowlist is bound into the plugin's host bindings at
  register time, together with its id. JS cannot read or change it.
  Test: `allowlist_cannot_be_changed_from_js`.
- [x] `check_url_allowed` enforces the allowlist on the initial request and on
  every redirect hop. Test:
  `domain_allowlist_is_enforced_per_plugin_and_per_hop` covers an allowed
  host, a non-declared host, a redirect to a non-declared host, and two
  plugins with different lists.
- [x] Install always requires approval: the server returns HTTP 428 without
  `approve_permissions`. The Web UI and `amigo-dl plugins install` show the
  domains first, or an explicit "any public host" warning.
- [x] An update whose domain set grows is never auto-applied. A manual update
  must be approved.
- [x] Before it is written into the plugin directory, a downloaded candidate is
  evaluated in a throw-away context. Its own `id` and `version` must match the
  signed registry entry, and its domains must not exceed what was approved. A
  rejected candidate never reaches disk, so it cannot reappear after a restart.
  An update replaces the installed file in place, including in category
  folders. Tests: `tests/updater_tests.rs`.
- A plugin without `permissions.domains` is *unscoped*. It keeps today's
  behaviour and is flagged in the UI. The default flips to deny once
  third-party plugins have migrated. `generic-http` and `xfilesharing` are
  unscoped by necessity.

## SH-5. Host-API version (#84)

- [x] `HOST_API_VERSION` (currently 1) and `MIN_SUPPORTED_API_VERSION` in
  `plugin-runtime/src/lib.rs` are the single source of truth.
- [x] A plugin declaring an unsupported major is refused at load, and the
  error names both versions (`Error::IncompatibleVersion`). A missing
  `apiVersion` loads as 1, with a warning, for one release.
- [x] `RegistryPlugin.api_version` lets the marketplace and the update check
  filter out entries the host cannot load.
- [x] Shipped plugins, the template, `amigo.d.ts` and the SDK declare
  `apiVersion`. `docs/plugin-api.md` states the compatibility policy.

## SH-6. Local bytecode cache (#83)

- [x] The cache lives at `$AMIGO_CONFIG_DIR/cache/plugin-bytecode/<key>.qjsc`.
  The key is `sha256(engine fingerprint || filename || source)`, and the
  fingerprint covers the QuickJS version, cache format, runtime version, and
  pointer width / endianness.
- [x] Each entry is `magic || sha256(payload) || payload`. A corrupted or
  truncated entry is deleted, and the plugin is recompiled from source in a
  fresh context. The cache is pruned LRU by mtime (256 entries).
- [x] Bytecode is never accepted remotely. The registry only installs `.ts`/`.js`
  source (`non_source_artifacts_are_rejected`).
- Measured with `tests/perf_baseline.rs` in release on the 6 shipped plugins
  (4 vCPU container):

  | | `discover()` |
  |---|---|
  | first cold start (SWC + compile) | 15.1 ms |
  | cold, transpile cache warm (median of 5) | 9.6 ms |
  | warm, bytecode cache (median of 5) | 6.4 ms |

  `generic-http` `resolve()` against a local mock (3 host HTTP calls) has a
  median of 0.84 ms over 50 runs.

---

## SH-7a. Out-of-process plugin host (#79) — process split

Plugin JS runs in a separate binary, `amigo-plugin-host`, not in the daemon.
This contains crashes (a QuickJS abort or segfault, or `panic = "abort"`, kills
the child, not the queue). It does **not** yet contain a memory-safety
exploit: the child runs as the same OS user with the same filesystem and
network reach as the daemon until SH-7b lands.

- [x] The daemon holds no `rquickjs::Runtime`. `PluginLoader` keeps metadata,
  URL matching, enable flags, per-plugin exec locks and the scoped `HostApi`s.
  The child (`plugin_host.rs`) owns every QuickJS runtime.
  Test: `daemon_runs_plugins_in_a_child_process`.
- [x] The child never touches the filesystem or the network. The parent reads
  plugin and spec sources and bytecode-cache entries and sends them; the child
  transpiles (SWC), evaluates and returns new bytecode for the parent to cache.
  Every stateful host function (HTTP, cookies, storage, notify, logging,
  captcha) is a callback to the parent; pure helpers (regex, crypto, URL,
  HTML) run in the child.
- [x] Host functions are registered from one backend-agnostic table
  (`host_fns::HOST_FNS`, B2 of `plugin-runtime-wasm-portability.md`). Its
  entries are `fn(&[Option<String>]) -> Result<Option<String>, String>` or a
  parent route; no engine type appears in a signature.
  Test: `table_covers_every_function_the_shim_uses`.
- [x] Protocol: length-prefixed (u32 BE) JSON over the child's stdin/stdout,
  max frame 64 MiB, a `hello` handshake that pins the protocol version.
  Tests: `ipc::tests::*`.
- [x] Every existing plugin test passes unchanged against the new boundary.
- [x] A child that exits (crash, abort, `SIGKILL`) fails only the in-flight
  calls with `Error::HostCrashed`, and marks those plugins `failing` (cleared
  on their next successful call). The next call spawns a new child; a plugin is
  re-loaded into it lazily from the source kept in memory. Cookies and storage
  live in the parent and survive. After 5 spawns within 60 s, spawning pauses
  for 30 s. Tests: `killed_child_fails_the_call_and_marks_the_plugin_failing`,
  `plugin_state_survives_a_child_restart`.
- [x] Liveness: a request whose child shows no activity for its budget plus a
  10 s grace, while no host callback is outstanding, gets the child killed.
  `solveCaptcha` is a callback, so a pending captcha never trips it.
  Tests: `stopped_child_is_killed_by_the_watchdog`,
  `captcha_callback_round_trips_through_the_parent`.
- [x] The parent finds the binary through `AMIGO_PLUGIN_HOST_BIN`, then next to
  its own executable (and one level up from `deps/` for test binaries). When
  it is missing, loading fails with a clear error and the daemon runs without
  plugins. Test: `missing_host_binary_is_a_clear_error`.
- [x] The Docker image and both release archives (CLI, server) ship
  `amigo-plugin-host`.
- A compromised child could return poisoned bytecode for the cache. That
  bytecode is only ever evaluated in a child again, so it adds persistence but
  no new reach. The parent re-validates everything it trusts from a manifest
  (allowlist, URL pattern, API version).

## SH-7b. OS sandbox for the plugin host (#79) — open

These decisions are settled here so the implementation PR does not have to
argue them again:

- **Topology:** one shared child process (`amigo-plugin-host`) for all
  plugins. The threat is plugin-vs-daemon. Cookie jars, storage, request
  counters and allowlists are already per plugin, and each plugin already has
  its own QuickJS runtime inside the child.
- **Sandbox:** on Linux, Landlock with no filesystem access plus a seccomp
  allowlist that denies `socket(2)`. On Windows, a Job Object with an
  AppContainer token. On macOS, a `sandbox_init` profile that denies `file*` and
  `network*`. The child needs neither: SH-7a already moved all I/O to the
  parent.
- **No sandbox primitive available** (for example, Linux before 5.13): refuse
  to run *unscoped* plugins and run scoped ones with a startup warning. An
  `AMIGO_PLUGIN_SANDBOX=off` override is for development only.
- **Measurements:** compare against the baseline in SH-6 (load and resolve)
  plus daemon startup.
