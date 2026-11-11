---
status: implemented
created: 2026-11-10
size: L
design: [AGENTS.md "Plugin-System", AGENTS.md "Wichtige Design-Entscheidungen" 14, docs/specs/plugin-runtime-wasm-portability.md, docs/specs/plugin-sandbox-hardening.md SH-7a]
surfaces: [core]
---

# Plugin Wasm guest: QuickJS as a WebAssembly module with an executor

## Context

Plugin JavaScript runs in QuickJS, a C interpreter with a history of
JS-reachable memory-safety bugs. Since SH-7a it runs in the child process
`krake-plugin-host`, which contains a crash but not an exploit: native code
in the child has the daemon user's file and network access (#79).

Decision 14 (2026-11-10) replaces the per-platform OS sandbox planned as SH-7b
with memory isolation: QuickJS is compiled to WebAssembly and executed by
`wasmtime` inside the plugin host. A memory-safety bug in QuickJS then corrupts
only the module's linear memory. The module has no system interface; the only
way out is the host-function table (`host_fns::HOST_FNS`).

This is the first of two specs. It delivers the module and an executor for it,
proven by its own tests. `plugin-wasm-sandbox` then makes the plugin host use
the executor for every plugin and removes native QuickJS from it.

Settled by a prototype (2026-11-10, 4 vCPU container, release build):

- `rquickjs` 0.14 builds for `wasm32-unknown-unknown` with the system `clang`
  18 and `wasm-ld`; the module is 0.85 MB and has one import (a clock).
- Compiling the module with Cranelift takes about 1.0 s, with Winch about
  0.17 s; loading a module precompiled at build time takes 1.5 ms.
- A loop of 3 million iterations runs in 0.29 s (Cranelift).
- QuickJS's own stack-limit check misfires in Wasm and must stay off; the
  Wasm runtime's stack limit takes its place.

## Scope

### In scope

- A guest module, built from source with the workspace, that contains QuickJS
  and the engine logic that today lives in `engine.rs` (evaluate a plugin,
  read its manifest exports, call `resolve` / `postProcess`, run a plugin
  spec, compile to and evaluate from QuickJS bytecode).
- An executor in `krake-plugin-runtime` that runs the guest in `wasmtime`,
  one instance per plugin, and offers the operations the plugin host needs.
- Host functions reach the guest only through the existing table
  (`HOST_FNS`): pure helpers run natively in the executor's process, parent
  routes go to a caller-supplied dispatcher.
- Limits per instance: memory, execution deadline, stack depth.
- Build tooling: the guest builds locally, in CI's Rust job and in the Docker
  image build. The five release targets are proven in `plugin-wasm-sandbox`
  (its AC-09), because only the release workflow builds them.

### Out of scope

- Using the executor in `krake-plugin-host` and removing native QuickJS from
  it — because that is `plugin-wasm-sandbox`; this spec stays testable on its
  own and within one size L.
- TypeScript transpilation inside the guest — because SWC is memory-safe Rust
  and stays native in the plugin host.
- The YouTube extractor's native QuickJS (`crates/extractors`,
  `n_challenge.rs`) — because it is a different code path with its own trust
  question; it is listed in the spec index backlog.
- Fuel metering and per-plugin CPU quotas — because the wall-clock deadline is
  the existing contract (`SandboxLimits`), and nothing asks for deterministic
  metering yet.
- Any change to the plugin API (`krake.*`, `krake.d.ts`, `HOST_API_VERSION`) —
  because plugins must not notice the engine change.

## Acceptance criteria

- [x] AC-01 (always): Krake's guest module imports nothing but functions
  the executor defines for it (the host-function call and a clock) and no
  WASI function; any other import fails a test.
- [x] AC-02 (event): When the executor loads a plugin source that exports a
  valid manifest and `resolve`, then it returns the same manifest fields
  (id, name, version, apiVersion, urlPattern, pluginType, permissions) the
  native engine reads from that source; when the source is invalid (syntax
  error, no `resolve`, a manifest field of the wrong type), then loading fails
  with the same error message as natively. Sources: every plugin under
  `plugins/` and the fixtures of this spec's tests.
- [x] AC-03 (event): When `resolve(url)` is called through the executor on a
  plugin that calls pure helpers (`krake.regexMatch`, `krake.sha256`,
  `krake.urlResolve`, `krake.htmlQueryAttr`) and parent-routed functions
  (`krake.httpGet`, `krake.storageSet`, `krake.storageGet`), then the
  dispatcher receives the parent-routed calls with the same qualified names
  and arguments as under the native engine, and the returned
  `DownloadPackage` is structurally equal JSON.
- [x] AC-04 (event): When `postProcess` or a plugin spec (`plugin.spec.ts`
  source, already transpiled) is run through the executor, then the result
  JSON / test results are structurally equal to the native engine's for the
  same input (the shipped `plugins/**/plugin.spec.ts` and a `postProcess`
  fixture).
- [x] AC-05 (event): When a plugin is compiled to bytecode through the
  executor and that bytecode is evaluated in a fresh instance, then its manifest
  and its `resolve` result equal those of the same plugin loaded from source.
- [x] AC-06 (error): If a host function returns an error, then the plugin
  sees a JS `Error` with that message and can catch it; if it does not catch
  it, the call fails with an execution error carrying the message.
- [x] AC-07 (error): If plugin code runs longer than its deadline (an
  infinite loop in the module body, in `resolve`, in a manifest getter, or a
  catastrophically backtracking JavaScript `RegExp`), then the call ends with
  a timeout error within the deadline plus 1 s, and the instance is unusable.
  The deadline is wall-clock per call; if it passes while the guest is blocked
  in a host call, the call ends with the timeout when that host call returns.
- [x] AC-08 (error): If plugin code allocates more than the instance's memory
  limit, then the call fails with an execution error, the process keeps
  running, the instance is unusable and other instances are unaffected.
- [x] AC-09 (error): If plugin code recurses without bound, then the call
  fails with an execution error, the process keeps running and the instance
  is unusable.
- [x] AC-10 (error): If the guest traps (an out-of-bounds memory access or
  `unreachable`, as a memory-safety bug in QuickJS would cause), then the call
  fails with an execution error, the instance is unusable, the process keeps
  running, and a new instance of the same plugin works.
- [x] AC-11 (always): Two instances share no JS state and no memory: a global
  set in one plugin is undefined in another, and each has its own deadline, so
  a plugin finishing does not disarm another's.
- [x] AC-12 (state): While one instance is blocked in a parent-routed host
  call, other instances run and complete.
- [x] AC-13 (always): Creating the executor (making the module ready to
  instantiate) takes under 50 ms, and creating one plugin instance under 5 ms,
  in a release build on the 4 vCPU container (`cargo test -p
  krake-plugin-runtime --release --test perf_baseline -- --ignored
  --nocapture`, median of 5; the ignored benchmark is run by hand like the
  SH-6 baseline, and its numbers go into Evidence).
- [x] AC-14 (always, CI: Rust job and Docker build): A clean checkout builds
  the plugin runtime, guest included, with the ordinary workspace build and
  no manual step, locally and in CI's Rust job and Docker build.
- [x] AC-15 (error): If the tools the guest build needs are missing (`clang`
  with a WebAssembly target, the `wasm32-unknown-unknown` Rust target), then
  the build fails with a message that names the missing tool and the command
  that installs it on Linux, macOS and Windows. The check is a function a
  unit test calls with an empty tool search path.
- [x] AC-16 (error): If the guest passes the executor a host-function index
  outside the table, a malformed argument frame, or a pointer/length
  outside its memory, then that call fails with an execution error and the
  process keeps running; nothing outside the instance is read or written.
- [x] AC-17 (error): If the embedded module cannot be made ready (it does not
  match the running CPU or `wasmtime`), then creating the executor returns an
  error that says so, without a panic.

## Technical notes

- **Guest crate.** A `cdylib` for `wasm32-unknown-unknown`. It is not a
  workspace member (the workspace `exclude`s it and it has its own
  `[workspace]` and `Cargo.lock`), so `cargo test --workspace` on the host
  target never selects it. `rquickjs` without the `parallel` and
  `bindgen` features (the target has no threads, and bindings ship
  pregenerated). `Runtime::set_max_stack_size(0)`; the memory limit is
  the instance's linear-memory limit, not `set_memory_limit`.
- **ABI.** Buffers cross as (pointer, length) into guest memory, with exported
  `alloc` / `dealloc`. One import carries every host call: function index
  into `HOST_FNS` plus a length-prefixed argument frame, returning a
  `HostResult` frame. One export handles a request frame: an operation
  (evaluate, read an export, compile, run bytecode, run a pending job) and
  its arguments. The guest is thin; the `resolve`, `postProcess` and spec
  scripts stay in `engine.rs` (see Decisions).
- **Embedding.** `build.rs` of `krake-plugin-runtime` builds the guest crate
  (nested `cargo build --target wasm32-unknown-unknown`, its own target
  directory under `OUT_DIR`) and precompiles it with the same `wasmtime`
  version for the target triple (`Config::target`), so start-up is a
  deserialize, not a compile (AC-13). `Module::deserialize` is `unsafe`: the
  bytes are produced by this build and embedded in the binary, which is the
  documented safe use; the `// SAFETY:` comment says so.
- **Terms.** "The process" is the one the executor runs in (a test binary
  here, the plugin host later). "Timeout error" is `Error::Timeout`,
  "execution error" `Error::Execution` of `krake-plugin-runtime`. An
  "unusable" instance returns an execution error on every further call; its
  owner drops it and loads the plugin again.
- **Limits.** Values come from `SandboxLimits` (64 MB linear memory per
  instance, which holds the QuickJS heap and the engine; 30 s per call, 5 s
  per load) and a 512 KiB Wasm stack.
- **Deadline.** Epoch interruption with a ticker thread; each store's
  deadline is set per call. The 1 s of AC-07 is the tick granularity plus
  scheduling slack.
- **Concurrency.** One `Store` per plugin; a call takes `&mut` on its
  instance, so two calls on one instance cannot overlap. Stores are
  independent, so plugins run in parallel as today.
- **Budgets.** AC-13's 50 ms and 5 ms are about 30 times the prototype's
  1.5 ms and 0.1 ms: loose enough for a slower machine, tight enough to fail
  if the module is compiled at start (about 1 s).
- **Toolchain.** `rust-toolchain.toml` gains the `wasm32-unknown-unknown`
  target; CI, `docker/Dockerfile` and `Cross.toml` install `clang` and `lld`.
  Windows and macOS release runners need LLVM with the WebAssembly target
  (Apple's `clang` has none: `brew install llvm`).
- **Dependencies** (approved with decision 14): `wasmtime` (features
  `cranelift`, `runtime`, `std` and what precompiling for other targets
  needs) in `krake-plugin-runtime`, also as a build dependency.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Plugin source with a syntax error | Load fails with the syntax error message, as natively; the instance is discarded |
| Host function result larger than the guest's free memory | The call fails with an execution error (AC-08), no trap of the process |
| A parent-routed call is made after the deadline passed while it was blocked | The call ends with a timeout once control returns to the guest |
| Corrupt or truncated bytecode is evaluated | The call fails with an execution error (QuickJS rejects it, or the guest traps as in AC-10); the caller falls back to the source, as the bytecode cache does today. Bytecode of another guest build is kept out by the cache key (`plugin-wasm-sandbox` AC-06) |
| The dispatcher returns an error or is gone | The host call fails like any host-function error (AC-06) |
| Empty plugin source | Load fails as natively: no manifest |
| Plugin throws a non-`Error` value (string, object) | The execution error carries its string form, as natively |
| Cross build (x86_64 host, aarch64 target) | The embedded precompiled module is for the target; the binary starts on the target without compiling |
| `resolve` returns a `Promise` | It is settled inside the guest's job queue and its value is returned, as natively |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The native engine stays in the tree during this spec as the reference for AC-02 to AC-05 | "Equal to native" is the cheapest precise oracle; `plugin-wasm-sandbox` removes it | Low: two engines exist for one spec's duration |
| Precompiling at build time is acceptable although it adds one `unsafe` call | The alternative is about 1 s on every plugin-host start, including every `krake <url>` run | Medium: a new `unsafe` is a stopping point; it is listed here for review |
| LLVM with a WebAssembly target can be installed on the Windows and macOS release runners | `choco install llvm` / `brew install llvm` provide it | Medium: only CI can prove it; AC-14 covers Linux, the release jobs are checked in `plugin-wasm-sandbox` |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `guest_imports_only_what_the_executor_defines` (crates/plugin-runtime/tests/wasm_guest_tests.rs) | verified | 48ce491 |
| AC-02 | `manifests_of_shipped_plugins_match_the_native_engine`, `invalid_sources_fail_with_the_native_message` (crates/plugin-runtime/tests/wasm_guest_tests.rs) | verified | 82955de |
| AC-03 | `resolve_makes_the_same_host_calls_and_returns_the_same_package` (crates/plugin-runtime/tests/wasm_guest_tests.rs) | verified | 82955de |
| AC-04 | `post_process_matches_the_native_engine`, `shipped_plugin_specs_give_the_native_results` (crates/plugin-runtime/tests/wasm_guest_tests.rs) | verified | 82955de |
| AC-05 | `bytecode_round_trip_behaves_like_the_source`, `corrupt_bytecode_is_an_error_not_a_crash` (crates/plugin-runtime/tests/wasm_guest_tests.rs) | verified | 82955de |
| AC-06 | `host_function_errors_are_catchable_js_errors`, `a_missing_return_value_reads_like_the_native_engine` (crates/plugin-runtime/tests/wasm_guest_tests.rs) | verified | 82955de |
| AC-07 | `runaway_code_ends_with_a_timeout`, `a_deadline_that_passes_during_a_host_call_ends_the_call_on_return` (crates/plugin-runtime/tests/wasm_guest_tests.rs) | verified | 82955de |
| AC-08 | `exceeding_the_memory_limit_fails_the_call_only` (crates/plugin-runtime/tests/wasm_guest_tests.rs) | verified | 82955de |
| AC-09 | `unbounded_recursion_fails_the_call_only` (crates/plugin-runtime/tests/wasm_guest_tests.rs) | verified | 82955de |
| AC-10 | `a_trapping_guest_is_contained` (crates/plugin-runtime/tests/wasm_guest_tests.rs) | verified | 82955de |
| AC-11 | `instances_share_no_state_and_keep_their_own_deadline`, `a_finishing_instance_does_not_disarm_another_deadline` (crates/plugin-runtime/tests/wasm_guest_tests.rs) | verified | 82955de |
| AC-12 | `a_blocked_host_call_does_not_block_other_instances` (crates/plugin-runtime/tests/wasm_guest_tests.rs) | verified | 82955de |
| AC-13 | `wasm_executor_startup_baseline` (crates/plugin-runtime/tests/perf_baseline.rs, ignored, run by hand): engine 1.6 ms, instance 0.8 ms (median of 5, three runs, release, 4 vCPU container, 2026-11-10); spec-check 2026-11-11: six runs, engine 1.6 to 5.2 ms, instance 0.87 to 3.1 ms (the slowest right after the build) | verified | 82955de |
| AC-14 | clean local build (`rm -rf target && cargo test -p krake-plugin-runtime`); CI: Rust job and Docker build green for 60f6e46 (PR #106); spec-check built the release profile from an empty `target/release` | verified | 9c3345f |
| AC-15 | `missing_tools_are_named_with_install_commands`, `clang_without_a_wasm_target_names_the_fix` (crates/plugin-runtime/src/guest_tools.rs) | verified | 82955de |
| AC-16 | `hostile_host_calls_are_rejected` (crates/plugin-runtime/tests/wasm_guest_tests.rs), `host_call_arguments_must_match_the_table_row` (crates/plugin-runtime/src/wasm.rs) | verified | 82955de |
| AC-17 | `foreign_bytes_are_refused_without_a_panic` (crates/plugin-runtime/src/wasm.rs) | verified | 82955de |

## Summary

> Completed: 2026-11-11

- `crates/plugin-guest`: QuickJS as a `wasm32-unknown-unknown` module with no
  system interface; it imports only the host-function call and a clock.
- `krake_plugin_runtime::wasm`: `WasmEngine` / `WasmContext` run the guest in
  `wasmtime`, one instance per plugin, with the operations of the native
  `PluginContext` and the same results and error messages.
- Limits per instance: linear memory (64 MB), wall-clock deadline per call
  (epoch interruption), 512 KiB Wasm stack; a call that hits one or traps
  leaves only that instance unusable.
- `build.rs` builds the guest and embeds it precompiled for the target, so
  start-up is a deserialize (about 2 ms); missing tools are named with their
  install commands. CI, the Docker image and `Cross.toml` install clang and lld.
- The plugin host does not use the executor yet: that is `plugin-wasm-sandbox`.

## Decisions

- 2026-11-10: `wasmtime` over `wasmi` — a JS interpreter inside a Wasm
  interpreter is an order of magnitude slower and would put CPU-heavy plugins
  at risk of the 30 s limit; `wasmtime` is the audited reference runtime.
- 2026-11-10: the guest is built from source in every build, not committed as
  a binary — the user had no preference; a build from source is reviewable
  and cannot drift from the code.
- 2026-11-10: `wasm32-unknown-unknown`, not WASI — the module then cannot even
  name a system call; `rquickjs-sys` ships the libc pieces it needs.
- 2026-11-10 (build): the guest is thin. It offers primitives (evaluate,
  read an export, compile, run bytecode, run a pending job); the scripts for
  `resolve`, `postProcess` and plugin specs stay in `engine.rs` and are shared
  by both engines, so their behavior cannot drift.
- 2026-11-10 (build): the host-function table and the JS shim are sent to
  the guest when an instance is created, so `HOST_FNS` stays the single
  source and the guest crate shares no code with the runtime.
- 2026-11-10 (build): frames are length-prefixed bytes, not JSON as the
  technical notes sketched — bytecode is binary and the guest then needs no
  JSON library.
- 2026-11-10 (build): the Wasm stack limit is 512 KiB (wasmtime's default),
  not the 1 MiB of the technical notes: plugin-host worker threads have 2 MiB
  of native stack, and the limit must stay well below it.
- 2026-11-10 (build): the deadline is wall-clock per call, as in the native
  engine: time blocked in a host call counts, and a call whose deadline
  passed while it was blocked times out when control returns to the guest.
- 2026-11-10 (build): the guest has a self-test operation (`trap`,
  `bad-index`, `bad-pointer`, `bad-frame`) that makes it misbehave on
  purpose. AC-10 and AC-16 need a guest that traps or passes hostile
  arguments, and no JavaScript can make a correct QuickJS do that. Only the
  executor can send operations; plugin code cannot reach it.
- 2026-11-10 (build): `unsafe` beyond the approved `Module::deserialize`: the
  guest's exported `alloc` / `dealloc` / `call`, its `host_call` import and
  the raw QuickJS calls for bytecode (moved from `bytecode_cache.rs`). All of
  it runs inside the Wasm instance; a pointer ABI is the only way across the
  boundary. For review.
- 2026-11-10 (build): tests were written after the first working build, not
  before: they need the embedded module to compile at all. Red runs that did
  happen: `runaway_code_ends_with_a_timeout` hung because the deadline ticker
  stopped when the engine was dropped before its instances (fixed: instances
  keep the ticker alive).
- 2026-11-10 (build): a host-function error reaches JS as an `Error` whose
  message carries rquickjs's prefix ("Error converting from js 'string' into
  type 'Error': …") — under both engines, so AC-06 compares them and checks
  that the message ends with the host's text.
- 2026-11-10 (build): the release workflow already gets the toolchain steps
  (wasm32 target, clang and lld on Linux, Homebrew LLVM on macOS), because the
  next release would fail without them. Their proof stays AC-09 of
  `plugin-wasm-sandbox`.
- New public API: `wasm::WasmEngine` (`new`, `create_context`,
  `guest_imports`, `defined_imports`), `wasm::WasmContext` (the operations of
  `engine::PluginContext`, plus `is_unusable`).
- 2026-11-10 (revision after review and the first acceptance round): AC-07
  and AC-16 reworded to what was built and tested (wall-clock deadline;
  argument frames, not JSON); technical notes brought in line with the
  Decisions above. No behavior changed, so their Evidence stays `built`.
- 2026-11-10 (acceptance round 1): the edge case "`resolve` returns a
  `Promise`" failed under both engines with `__resolve_done is not defined` —
  a bug already on `main`: the resolve script kept its state in function
  locals, the poll read globals. Fixed in the shared script; regression test
  `a_promise_from_resolve_is_settled_under_both_engines` (crates/plugin-runtime/tests/wasm_guest_tests.rs), red before
  the fix.
- 2026-11-10 (acceptance round 1): tests widened — the template plugin in the
  manifest comparison (AC-02), all seven manifest fields after a bytecode
  round trip (AC-05), a finishing instance not disarming another's deadline
  (AC-11).
- 2026-11-10 (acceptance round 2): a value-less result (`resolve` without a
  `return`) gave "the plugin engine returned no value" instead of the native
  conversion error; the guest now converts like the native engine, pinned by
  `a_missing_return_value_reads_like_the_native_engine`. A timeout between
  two steps of a Promise-returning `resolve` now also leaves the instance
  unusable. Test added for a deadline that passes during a host call (AC-07).
- 2026-11-10 (acceptance round 3): a thrown non-`Error` object showed as
  Rust's debug form with a memory address (`Object(0x…)`), under both
  engines and already on `main`. Both now render `String(value)`; test
  `thrown_non_errors_carry_their_string_form_under_both_engines`. Round 3 was
  the last allowed verification round, so the spec stays `todo` until
  `spec-check` is run again.
