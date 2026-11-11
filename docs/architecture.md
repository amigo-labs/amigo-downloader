# Architecture

See AGENTS.md for the full architecture overview.

## Plugin isolation

Decision 14 in `AGENTS.md`. Three layers, from the inside out:

1. **WebAssembly instance per plugin.** QuickJS is compiled to
   `wasm32-unknown-unknown` (`crates/plugin-guest`) and executed by `wasmtime`
   (`crates/plugin-runtime/src/wasm.rs`). An instance has no system interface;
   it can compute, read a clock and call the host-function table. A
   memory-safety bug in QuickJS stays in that instance's linear memory. Limits
   per instance: memory, wall-clock deadline, stack. A call that hits one, or
   traps, fails alone and its instance is replaced on the next call.
2. **Plugin host process.** All instances live in `krake-plugin-host`, a child
   of the daemon (or of `krake`). It does no I/O of its own: sources and
   bytecode come from the daemon, and every stateful host function (HTTP,
   cookies, storage, captcha) is a callback to it. A crash kills only this
   process; the daemon restarts it and re-loads plugins lazily.
3. **Host API in the daemon.** Per-plugin domain allowlist, request budget,
   storage quota and SSRF checks, enforced where the network and the data are.

The guest is built by `crates/plugin-runtime/build.rs` and embedded
precompiled, so the plugin host needs no file besides its executable and no
compiler at run time. There is no native execution path.

Specs: `docs/specs/plugin-wasm-guest.md`, `docs/specs/plugin-wasm-sandbox.md`;
the hardening that came before: `docs/specs/plugin-sandbox-hardening.md`.
