# Plugin Sandbox Hardening

**Date**: 2026-10-02
**Status**: Implemented (#78, #80–#84), design note (#85), open (#79)
**Scope**: `crates/plugin-runtime`, server plugin endpoints, Web UI / CLI install flow, release workflow
**Tracking**: #77

---

## Summary

The plugin sandbox's documented limits (30 s, 64 MB, 20 requests, 1 MB storage)
bounded a *cooperating* interpreter but were not a security boundary: some
entry points ran without a deadline, the registry signature check could be
bypassed, and any plugin could reach any host. This spec covers the in-process
hardening. Process isolation (#79) is the remaining step that changes the trust
model; its design decisions are recorded at the end.

---

## SH-1. Registry signature verification is load-bearing (#80)

**Acceptance criteria**

- [x] No construction path can set the trusted key to the all-zero placeholder.
  `RegistryConfig::trusted_signing_key` is private; the server uses
  `RegistryConfig::for_index`, derived from `Default`;
  `with_trusted_signing_key` rejects low-order keys.
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
  must be approved. If a plugin's own manifest claims more than was approved,
  it is disabled.
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

## SH-7. Out-of-process plugin host (#79) — open

These decisions are settled here so the implementation PR does not have to
argue them again:

- **Topology:** start with one shared child process (`amigo-plugin-host`) for
  all plugins. The threat is plugin-vs-daemon. Cookie jars, storage, request
  counters and allowlists are already per plugin, and each plugin already has
  its own QuickJS runtime inside the child.
- **Protocol:** length-prefixed JSON over the child's stdin/stdout. Messages
  are `load`, `resolve`, `postProcess` and `runSpec`, plus host-API callbacks
  routed back to the parent. Every host function already fits
  `(&str, …) -> Result<String, String>`; see `plugin-runtime-wasm-portability.md`.
  The `HostApi` (HTTP, SSRF and domain checks, storage, captcha) stays in the
  parent, so the child needs no network access.
- **Long-blocking callbacks:** `solveCaptcha` is a callback that has no IPC
  request timeout. The wall-clock invocation deadline is enforced in the child
  by the interrupt handler, as it is today. The parent only watches the child
  for liveness.
- **Sandbox:** on Linux, Landlock with no filesystem access plus a seccomp
  allowlist that denies `socket(2)`. On Windows, a Job Object with an
  AppContainer token. On macOS, a `sandbox_init` profile that denies `file*` and
  `network*`.
- **No sandbox primitive available** (for example, Linux before 5.13): refuse
  to run *unscoped* plugins and run scoped ones with a startup warning. An
  `AMIGO_PLUGIN_SANDBOX=off` override is for development only.
- **Fault handling:** if the child crashes, that resolve fails, the child is
  restarted and the plugin is marked failing. The download queue keeps
  running.
- **Measurements:** compare against the baseline in SH-6 (load and resolve)
  plus daemon startup.
