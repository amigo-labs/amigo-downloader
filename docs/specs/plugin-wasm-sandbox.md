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
  embedded module), then it is never read: the plugin loads from source and
  this build writes its own entry; when an entry of this build is corrupt,
  then it is dropped, the plugin loads from source and the entry is replaced.
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
| A plugin uses `Atomics` | The global does not exist in the guest (it is built without threads); no shipped plugin uses it; documented in `docs/plugin-api.md` |
| A plugin kept state in a JS global and then hit a limit | Its next call runs in a fresh instance: the global is gone; cookies and storage (in the daemon) survive; documented in `docs/plugin-api.md` |
| A plugin throws an ordinary error | The call fails with that error; the instance stays and the plugin is not marked `failing` |
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
| AC-01 | `the_plugin_runtime_does_not_depend_on_native_quickjs` (crates/plugin-runtime/tests/wasm_sandbox_tests.rs); the plugin host only links `wasm.rs` for execution | built |        |
| AC-02 | existing suites unchanged and green: `runtime_tests`, `sandbox_tests`, `hardening_tests`, `host_api_tests`, `process_host_tests`, `spec_runner`, `updater_tests`; `wasm_guest_tests` against `tests/fixtures/native_engine_expectations.json` | built |        |
| AC-03 | `a_trap_or_timeout_fails_one_call_and_the_plugin_recovers`, `other_plugins_calls_complete_while_one_traps`, `a_trap_or_timeout_while_loading_leaves_the_plugin_unregistered`, `an_ordinary_plugin_error_does_not_mark_the_plugin_failing` (crates/plugin-runtime/tests/wasm_sandbox_tests.rs) | built |        |
| AC-04 | `exceeding_the_memory_limit_fails_the_call_and_the_next_one_works` (crates/plugin-runtime/tests/wasm_sandbox_tests.rs) | built |        |
| AC-05 | AC-02 suites (timeouts, HTTP budget, storage quota in `hardening_tests`, `host_api_tests`, `sandbox_tests`) plus AC-04 | built |        |
| AC-06 | `second_load_comes_from_bytecode_cache_and_corruption_falls_back` (crates/plugin-runtime/tests/hardening_tests.rs), `bytecode_of_another_guest_build_is_never_read` (crates/plugin-runtime/tests/wasm_sandbox_tests.rs), `engine_version_is_part_of_the_key` (crates/plugin-runtime/src/bytecode_cache.rs) | built |        |
| AC-07 | `plugin_load_and_resolve_baseline` (crates/plugin-runtime/tests/perf_baseline.rs, ignored, run by hand, asserts the budgets in release): cold 21 ms, warm 16 ms, resolve 1.9 ms (medians, three runs, release, 4 vCPU container, 2026-11-11) | built |        |
| AC-08 | `the_plugin_host_needs_only_its_own_executable` (crates/plugin-runtime/tests/wasm_sandbox_tests.rs); archives and image: `.github/workflows/release-build.yml` and `docker/Dockerfile` ship the same three binaries as before | built |        |
| AC-09 | CI: release builds — the next run of `.github/workflows/release-build.yml` | open |        |
| AC-10 | manual: daniel-rck checked the sandbox documentation (`docs/plugin-api.md` (Sandbox Limits, bytecode), `docs/architecture.md` (Plugin isolation), AGENTS.md (Sandboxing, Tech Stack), README, `docs/site/project.yml`, `plugin-sandbox-hardening.md`, `plugin-runtime-wasm-portability.md`), 2026-11-11 | built |        |

## Decisions

- 2026-11-10: Wasm replaces the OS sandbox of SH-7b (user decision): one
  mechanism on Windows, macOS and Linux, testable in CI, no per-platform
  sandbox code.
- 2026-11-10: the child process stays, and there is no native fallback — the
  user had no preference; two layers and one execution path are the safer
  default.
- 2026-11-11 (build): the host API is bound after the module body ran, as
  before: the guest got a separate bind operation, so a module body still
  cannot call `krake.*`.
- 2026-11-11 (build): an instance that became unusable is rebuilt inside the
  plugin host on the plugin's next call, from the source the host keeps
  (module body evaluated again, host API bound). The daemon learns of it
  through a new `instance_lost` flag on the response and marks the plugin
  `failing` until its next successful call. An ordinary JS error does neither.
- 2026-11-11 (build): IPC protocol version 2 (the `instance_lost` flag, and
  bytecode that is not interchangeable with version 1).
- 2026-11-11 (build): the bytecode cache key is the SHA-256 of the embedded
  guest module plus the cache format and crate version. Entries of another
  build are never read and leave with the cache's size bound; AC-06 reworded
  to say that instead of "replaced" (no behavior was dropped).
- 2026-11-11 (build): `EngineConfig::max_stack_size` is gone: the Wasm stack
  limit is fixed at 512 KiB by the engine configuration both `build.rs` and
  the runtime use.
- 2026-11-11 (build): the unit tests of the removed native engine in
  `engine.rs` (4) went with it; they tested code that no longer exists.
  Their cases (context creation, eval, timeout of `postProcess`) are covered
  by `wasm_guest_tests`.
- 2026-11-11 (build): the comparison tests of `plugin-wasm-guest` replay the
  native engine's answers from `tests/fixtures/native_engine_expectations.json`,
  recorded at the last commit that had both engines.
- 2026-11-11 (build): tests of this spec were written after the switch, not
  before; the existing suites were the red/green signal for the switch
  itself (they ran green at the first full run).
- 2026-11-11 (measurements, release, 4 vCPU container, medians of three runs):

  | | plugin host, native (SH-7a) | plugin host, Wasm |
  |---|---|---|
  | `discover()` cold (median of 5) | 17.3 ms | 21 ms |
  | `discover()` warm, bytecode cache (median of 5) | 11.9 ms | 16 ms |
  | `generic-http` `resolve()`, 3 host HTTP calls | 1.78 ms | 1.9 ms |

  `krake-plugin-host` is 9.3 MB in release.
