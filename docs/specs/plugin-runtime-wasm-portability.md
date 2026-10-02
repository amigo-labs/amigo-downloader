# Plugin Runtime: Keeping the Host-API Boundary Wasm-Portable

**Date**: 2026-10-02
**Status**: Design note (analysis only, no implementation) — #85
**Scope**: the boundary between plugin JS and `crates/plugin-runtime/src/host_api.rs`

---

## Why this note exists

Running QuickJS compiled to Wasm inside `wasmtime` (the Javy approach) would
provide three properties the in-process engine cannot:

- **Memory isolation.** A QuickJS heap overflow corrupts Wasm linear memory, not
  the daemon.
- **Fuel metering.** A deterministic instruction budget replaces the wall-clock
  interrupt.
- **Capabilities as explicit imports.** Host functions become declared imports.

The cost is roughly 2× JS execution time plus a toolchain dependency. This note
does not propose the switch. It records what the current boundary must avoid
so that a later switch is a port and not a rewrite.

## 1. The host-function table

All 44 functions registered by `register_host_api`, plus `console.*`, are
listed below. The `amigo.*` names a plugin sees that are not in the table
(`httpGet`, `httpGetJson`, `htmlQueryAll`, `traverse`, …) are pure-JS wrappers
in `JS_SHIM`. They never cross the boundary.

| Function | Arguments | Returns | Error |
|---|---|---|---|
| `__rawHttpGet` / `__rawHttpHead` / `__rawHttpGetBinary` / `__rawHttpFollowRedirects` | url, headers_json? | JSON / string | throws |
| `__rawHttpPost` | url, body, content_type, headers_json? | JSON | throws |
| `__rawHttpPostForm` | url, fields_json, headers_json? | JSON | throws |
| `setCookie` | domain, name, value | — | — |
| `getCookie` | domain, name | string? | — |
| `clearCookies` | domain | — | — |
| `storageGet` | key | string? | throws |
| `storageSet` | key, value | — | throws (quota) |
| `storageDelete` | key | — | — |
| `regexMatch` / `regexReplace` / `__rawSearchJson` | 2–3 strings | string? | — (null) |
| `regexMatchAll` | pattern, text | string[] | — |
| `regexTest` | pattern, text | bool | — |
| `__rawRegexSplit` | pattern, text | JSON | — |
| `base64Encode` / `md5` / `sha1` / `sha256` / `sanitizeFilename` | 1 string | string | — |
| `base64Decode` | input | string | throws |
| `hmacSha256` | key, data | string | throws |
| `aesEncryptCbc` / `aesDecryptCbc` | data, key, iv | string | throws |
| `urlResolve` | base, relative | string | throws |
| `__rawUrlParse` | url | JSON | throws |
| `urlFilename` / `htmlExtractTitle` | 1 string | string? | — |
| `__rawHtmlQueryAll` / `__rawHtmlHiddenInputs` / `__rawHtmlQueryAllAttrs` | 1–3 strings | JSON | throws |
| `htmlQueryText` / `htmlQueryAttr` | 2–3 strings | string? | throws |
| `__rawHtmlSearchMeta` | html, names_json | string? | — |
| `parseDuration` | input | number? | — |
| `notify` | title, message | — | — |
| `logInfo` / `logWarn` / `logError` / `logDebug`, `console.log/warn/error` | msg | — | — |
| `__rawSolveCaptcha` | image_url, type? | string | throws |

**Result: confirmed.** Every function fits `(&str, …) -> Result<String, String>`
under one encoding rule:

- `Option<String>` becomes nullable.
- `Vec<String>`, `bool` and `f64` are returned as JSON text, and the shim
  parses them.
- Errors are a message string that the binding rethrows as a JS `Error`.

No function takes or returns an engine handle (`Ctx`, `Value`, `Object`,
`Function`).

## 2. The blockers, and the smallest change for each

| # | Blocker | Smallest change | Worth doing now? |
|---|---|---|---|
| B1 | Host calls block via `block_in_place` + `Handle::block_on` | **Done in #82.** Execution runs on `spawn_blocking`, so a host call blocks a dedicated thread, not an executor worker. A Wasm host would call the same async functions from its own blocking thread, or via `wasmtime`'s async support. | Done |
| B2 | `register_host_api` is shaped around `rquickjs` (`&Ctx`, `Function::new`) | Extract a backend-agnostic table `[(name, arity, fn(&HostScope, &[&str]) -> Result<String, String>)]` and keep the `rquickjs` registration as a thin loop over it. #79's IPC needs the same table for the child-to-parent callbacks. | **Yes, as the first step of #79**: one table serves IPC and any future backend |
| B3 | `call_resolve` drives the job queue by `eval`-ing generated JS and polling `__resolve_done` globals | Replace with direct `Function::call` on the export plus `Promise` inspection through the engine API. Only the "call an export with a JSON argument, get JSON back" contract has to survive. | Only when a second backend exists. It is isolated in one method today. |
| B4 | `solveCaptcha` blocks on a human with no bound | Turn it into an out-of-band completion: `resolve()` returns a "needs captcha" result, the host solves it, and then resumes or re-invokes. This changes the plugin contract, so it needs a host-API major bump (see `docs/plugin-api.md`). | Not as part of the Wasm question. It changes the API. Separately, the wall-clock deadline keeps running while a captcha is pending, so a captcha answered after the 30 s budget still ends the resolve. That is a functional reason to fix B4 on its own. |
| B5 | A runtime-wide wall-clock deadline (shared atomic) | **Done in #81/#82.** The deadline is per plugin runtime, so the behaviour is already per instance, like fuel. | Done |

## 3. Measured baseline

From `crates/plugin-runtime/tests/perf_baseline.rs`, release build, 4 vCPU
container, quickjs-ng 0.16.2:

- `discover()` of the 6 shipped plugins: first cold start 15.1 ms, cold with
  the transpile cache warm 9.6 ms, warm from the bytecode cache 6.4 ms.
- `generic-http` `resolve()` against a local mock with 3 host HTTP calls:
  median 0.84 ms.

Real resolves are dominated by network latency: tens to hundreds of
milliseconds per request, and up to 20 requests. Even a 2× (or 5×) JS slowdown
would be noise end to end. The "~2×" figure is therefore not the deciding
cost. The toolchain and maintenance cost is.

## 4. Invariant to adopt now

> **No engine-specific type appears in a host-API function signature.** Host
> functions take `&str` arguments plus the scoped `HostApi` and return
> `Result<String, String>` (or a type with an obvious JSON encoding). Engine
> types stay in the registration adapter and in `engine.rs`.

Enforcement:

- **Today:** code review. This note is the reference.
- **With B2:** the function table's type signature enforces it, because the
  table cannot hold a closure that mentions `Ctx<'_>`.

## 5. Recommendation

Do not switch now. #79 (out-of-process host with OS sandboxing) also contains
the memory-safety bug class, at lower cost and without slowing JS down. With
B1 and B5 already done and B2 planned as part of #79, a later Wasm port stays
mechanical.

Revisit if any of these becomes true:

- Third-party plugins are accepted into the registry without amigo-labs review.
- A QuickJS CVE is found that #79's OS sandbox would not contain, for example a
  sandbox-escape primitive on a platform without Landlock or seccomp.
- #79 proves infeasible on a supported platform.
- Deterministic metering is needed, for example per-plugin CPU quotas in a
  multi-tenant deployment.
