---
status: todo
created: 2026-11-10
size: L
design: [AGENTS.md "Plugin-System", AGENTS.md "Wichtige Design-Entscheidungen" 14, docs/specs/plugin-sandbox-hardening.md SH-7a and SH-7b, docs/specs/plugin-wasm-guest.md]
surfaces: [core, cli, docker]
---

# Plugin Wasm sandbox: every plugin runs in the Wasm guest

## Context

`plugin-wasm-guest` delivers QuickJS as a WebAssembly module and an executor
for it. This spec makes it the only way plugin JavaScript runs: the plugin
host (`krake-plugin-host`) executes every plugin in a Wasm instance, native
QuickJS leaves the plugin runtime, and the documentation says what the
boundary now is. It replaces SH-7b (per-platform OS sandbox) of
`plugin-sandbox-hardening.md` and closes #79 and its tracker #77
(decision 14, 2026-11-10).

The process split of SH-7a stays as the second layer: a bug in the Wasm
runtime or a crash still hits only the child.

Depends on: `plugin-wasm-guest` (`status: implemented`).

## Scope

### In scope

- `krake-plugin-host` runs load, resolve, post-processing and plugin specs
  through the Wasm executor.
- Native QuickJS (`rquickjs` on the host target) is removed from
  `krake-plugin-runtime`.
- The bytecode cache keeps working and is keyed so that bytecode from another
  guest build is never evaluated.
- Release archives and the Docker image still ship exactly one extra binary,
  `krake-plugin-host`, with the guest embedded; the self-updater therefore
  needs no change.
- The tests of `plugin-wasm-guest` that compare against the native engine
  keep their expected values as recorded fixtures.
- Measurements against the SH-7a table, recorded in this spec.
- Documentation: SH-7b marked as replaced, `docs/plugin-api.md` sandbox
  section, `docs/architecture.md`, the plugin feature text in
  `docs/site/project.yml`, AGENTS.md "Sandboxing".

### Out of scope

- A switch that runs plugins natively (`KRAKE_PLUGIN_SANDBOX=off`) — because
  one execution path is what makes the boundary trustworthy; a switch that
  turns the sandbox off would have to be tested and defended forever.
- Moving plugin execution back into the daemon — because the process split is
  the second layer and costs nothing to keep.
- An OS sandbox around the child in addition — because Wasm removes the need
  it was planned for. An escape from the Wasm runtime itself would again have
  the child's rights; that is accepted, because such escapes are far rarer
  than QuickJS bugs, and it can be reconsidered if one is found.
- The YouTube extractor's native QuickJS (`crates/extractors`) — because it is
  not plugin code; it is in the spec index backlog.
- Changes to the plugin API surface (`krake.*`, `krake.d.ts`,
  `HOST_API_VERSION`) — because plugins must not notice the engine change.
  The one behavior that differs is unbounded recursion (edge cases).

## Acceptance criteria

- [ ] AC-01 (always): Krake runs every plugin entry point (module load,
  manifest read, `resolve`, `postProcess`, plugin specs) inside a Wasm instance; the plugin host process contains no native
  QuickJS. A test asserts that `krake-plugin-runtime` has no `rquickjs`
  dependency for the host target (`cargo tree -p krake-plugin-runtime
  --target <host>`).
- [ ] AC-02 (always): Every existing test of `krake-plugin-runtime`
  (`runtime_tests`, `sandbox_tests`, `hardening_tests`, `host_api_tests`,
  `process_host_tests`, `spec_runner`, `updater_tests`) and every shipped
  plugin spec (`plugins/**/plugin.spec.ts`) passes without a change to its
  assertions, and the tests of `plugin-wasm-guest` pass against recorded
  expected values instead of the removed native engine.
- [ ] AC-03 (error): If plugin code makes the guest trap or time out during a
  call, then only that call fails (execution error or timeout error), the plugin is marked `failing` until its
  next successful call, the child process keeps running, and other plugins'
  in-flight calls complete. If it happens while the plugin is being loaded,
  loading fails and the plugin is not registered.
- [ ] AC-04 (error): If a plugin exceeds its memory limit
  (`SandboxLimits::max_memory_bytes`), then the call fails with an execution
  error and the next call of the same plugin works in a fresh instance.
- [ ] AC-05 (always): The limits of `SandboxLimits` keep their meaning: 30 s
  per call, 5 s per module load, 64 MB per plugin, 20 HTTP requests per call,
  1 MB storage. The 64 MB bound the instance's linear memory (QuickJS heap
  plus engine). The existing tests for each stay green (AC-02), and the
  memory limit has a test of its own (AC-04).
- [ ] AC-06 (event): When a plugin is loaded a second time with an unchanged
  source, then it loads from the bytecode cache (observable as in the existing test
  `second_load_comes_from_bytecode_cache_and_corruption_falls_back`); when the
  cached entry was written by a different guest build (another hash of the
  embedded module) or is corrupt, then it is not evaluated, the plugin loads
  from source and the entry is replaced.
- [ ] AC-07 (always): `discover()` of the shipped plugins takes under 60 ms
  cold and under 40 ms warm (without and with the bytecode cache, as
  `perf_baseline` defines them), and a `generic-http` `resolve()` with 3 host HTTP
  calls under 5 ms, in a release build on the 4 vCPU container
  (`cargo test -p krake-plugin-runtime --release --test perf_baseline --
  --ignored --nocapture`, median of 5). The measured values are recorded under
  Decisions next to the SH-7a numbers.
- [ ] AC-08 (always): The CLI archive, the server archive and the Docker image
  contain `krake`/`krake-server` and `krake-plugin-host` and nothing else new;
  the plugin host needs no file besides its own executable at run time: a
  test copies it alone into an empty directory and runs a plugin through it.
- [ ] AC-09 (always, CI: release builds): The release workflow builds the
  plugin host with the embedded guest for all five targets (Linux x86_64 and
  aarch64, macOS x86_64 and aarch64, Windows x86_64).
- [ ] AC-10 (always, manual: documentation review): The documentation states
  the boundary as it is:
  `docs/specs/plugin-sandbox-hardening.md` marks SH-7b as replaced by this
  spec, `docs/plugin-api.md` and AGENTS.md "Sandboxing" say that plugin JS
  runs in a Wasm instance inside the plugin host process and no longer call
  the OS sandbox open, `docs/architecture.md` and the plugin feature text in
  `docs/site/project.yml` match, and
  `docs/specs/plugin-runtime-wasm-portability.md` records that the switch
  happened.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A plugin blocks in `solveCaptcha` for minutes | The call waits as today; the watchdog does not kill the child, other plugins run |
| The child is killed while calls are in flight | As SH-7a: those calls fail with `HostCrashed`, the next call spawns a new child and re-loads plugins lazily |
| A daemon updated on disk but not restarted respawns the plugin host | The handshake refuses a host with another protocol version; plugins fail closed until the restart |
| Bytecode cache directory holds entries from the native engine | They are ignored and replaced (AC-06); no migration step |
| A plugin relied on QuickJS's own stack-overflow `RangeError` being catchable | Unbounded recursion now ends the call with an execution error; documented in `docs/plugin-api.md` |
| The same plugin is called twice at once | The calls run one after the other on its instance, as today (per-plugin exec lock) |
| The bytecode cache directory is not writable | Plugins load from source every time; a warning is logged, as today |
| 6 plugins resolve at once | Each runs in its own instance in parallel; none waits for another's lock |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The IPC protocol version is bumped | The child's behavior behind the same messages changes, and the bytecode it returns is not interchangeable | Low: the handshake already handles a mismatch |
| The budgets of AC-07 leave room over the SH-7a numbers (17 ms cold, 12 ms warm, 1.8 ms resolve) | The prototype measured 1.5 ms to load the precompiled module and roughly 2 to 3 times slower JS | Medium: `resolve` is dominated by IPC (about 0.2 ms per host call), not JS, so 5 ms is roughly 3 times today's value; cold and warm are about 3 times today's. If a budget is missed, the spec is revised with the measurement, not the test weakened |
| Unbounded recursion ending the call instead of throwing a catchable error is acceptable | No shipped plugin relies on catching it, and the registry holds only reviewed plugins; the Wasm stack limit is the only reliable guard | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |

## Decisions

- 2026-11-10: Wasm replaces the OS sandbox of SH-7b (user decision): one
  mechanism on Windows, macOS and Linux, testable in CI, no per-platform
  sandbox code.
- 2026-11-10: the child process stays, and there is no native fallback — the
  user had no preference; two layers and one execution path are the safer
  default.
