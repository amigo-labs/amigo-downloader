# Rust 1.99 Toolchain and Modern-Idiom Baseline

**Date**: 2026-10-02
**Status**: Implemented
**Scope**: Pin the Rust toolchain to 1.99.0 across local, CI and Docker builds; move the workspace onto language/std features stabilized up to 1.99; enforce the result with a workspace lint table.

---

## Summary

The workspace built with three different compilers: CI followed the floating
`dtolnay/rust-toolchain@stable`, the Docker builder stage was pinned to
`rust:1.94-bookworm`, and local builds used whatever `rustup` default the
developer had. There was no `rust-toolchain.toml`. Builds were therefore not
reproducible, and new clippy lints could turn CI red without any code change
(clippy 1.99's `double_must_use` did exactly that on `async-trait` 0.1.89).

Independently, the code still carried idioms that newer Rust replaces:
`#[allow]` instead of `#[expect]`, `match { .. => return }` instead of
let-else, `assert!(matches!(..))`, `#[async_trait]` on a trait that is never
used as `dyn`, a hand-written boxed-future wrapper for recursion, hand-masked
IPv6 ranges, `Duration::from_secs(60 * 60)`, and so on.

---

## Design

### 1. Single toolchain pin

- `rust-toolchain.toml` — `channel = "1.99.0"`, components `rustfmt` and
  `clippy`. rustup reads it automatically and installs the toolchain on first
  use.
- CI (`ci.yml`, `release-build.yml`) — every `dtolnay/rust-toolchain` step
  uses `@1.99.0`.
- `docker/Dockerfile` — builder stage `FROM rust:1.99.0-bookworm`. The
  toolchain file is not copied into the image; the tag is the source of truth
  there.
- No `rust-version` (MSRV) is declared: the project ships binaries, so the
  pinned toolchain is what matters.

### 2. Modern-idiom lint guard

`[workspace.lints.clippy]` in the root `Cargo.toml`, opted into by every
member via `[lints] workspace = true`. Every listed lint flags code that has a
newer, stable replacement (clippy's default groups already cover the rest,
e.g. `collapsible_if` → let chains):

| Lint | Replacement it enforces |
|---|---|
| `allow_attributes`, `allow_attributes_without_reason` | `#[expect(.., reason = "..")]` (1.81) |
| `non_std_lazy_statics` | `std::sync::LazyLock` instead of `once_cell` / `lazy_static!` |
| `manual_let_else` | let-else |
| `manual_is_variant_and`, `manual_ok_or`, `manual_midpoint` | `is_some_and`, `ok_or`, `midpoint` |
| `uninlined_format_args` | `format!("{x}")` |
| `borrow_as_ptr`, `ref_as_ptr` | `&raw const` / `ptr::from_ref` |
| `duration_suboptimal_units` | `Duration::from_mins` / `from_hours` (1.91) |
| `unnested_or_patterns` | `Some(A \| B)` |
| `unchecked_time_subtraction` | no panicking `Instant - Duration` |

### 3. Code changes

- **async-trait**: `Extractor` (never used as `dyn`) declares
  `fn extract(..) -> impl Future<Output = ..> + Send`; implementations stay
  `async fn` and `async-trait` is dropped from `amigo-extractors`.
  `ProtocolBackend` and `UrlResolver` are used as `dyn` and keep
  `async-trait` (updated to 0.1.92, which no longer emits the bare
  `#[must_use]` that clippy 1.99 rejects).
- **Recursive async**: `GenericExtractor::extract_from_html` is a plain
  `async fn`; only the recursive call into iframes is `Box::pin`ned (1.77).
- **Async closures** (1.85): retry tests and the coordinator cancel test use
  `async |attempt| { .. }`, which borrow from their environment instead of
  cloning `Arc`s per attempt. `retry_with_policy` keeps its
  `FnMut(u32) -> Fut` bound: with an `AsyncFnMut` bound the spawned
  coordinator future fails to prove `Send` ("implementation of `Send` is not
  general enough").
- **Server binary on the library crate**: `main.rs` used to re-declare every
  module that `lib.rs` also declares, so each module compiled twice and
  `dead_code` disagreed between the two targets (hence a crate-wide
  `#![allow(dead_code)]`). `main.rs` now imports `amigo_server::*`; the
  crate-wide allow and three never-constructed JSON-RPC structs are gone.
- **std APIs**: let chains, let-else, `is_some_and` / `is_ok_and` /
  `is_none_or`, `bool::ok_or_else` (1.98), `String::from_utf8_lossy_owned`
  (1.99), `Duration::from_hours` / `from_mins` (1.91),
  `Ipv6Addr::is_unique_local` / `is_unicast_link_local`, `assert_matches!`
  (1.96), `cargo::` build-script directives.

---

## Acceptance Criteria

| ID | Criterion | How to verify |
|---|---|---|
| AC-1 | The repo builds with exactly Rust 1.99.0 | `rustc --version` inside the repo prints `rustc 1.99.0` |
| AC-2 | CI does not float on `@stable` | `grep -rn 'rust-toolchain@stable' .github` returns no matches |
| AC-3 | Docker builds with the same toolchain | `grep -n '^FROM rust:' docker/Dockerfile` shows `rust:1.99.0-bookworm` |
| AC-4 | The workspace is clean under the lint table | `cargo clippy --workspace --exclude amigo-desktop --all-targets --locked -- -D warnings` exits 0 |
| AC-5 | No `#[allow]` remains in first-party Rust code | `grep -rnE '#!?\[allow\(' crates tauri --include=*.rs` returns no matches |
| AC-6 | Every workspace member opts into the lint table | each `crates/*/Cargo.toml` and `tauri/Cargo.toml` contains `[lints]` with `workspace = true` |
| AC-7 | `Extractor` no longer depends on `async-trait` | `grep -n 'async-trait' crates/extractors/Cargo.toml` returns no matches |
| AC-8 | Tests pass | `cargo test --workspace --exclude amigo-desktop --locked` exits 0 |

---

## Out of Scope

- Dropping `async-trait` from `ProtocolBackend` / `UrlResolver` — both are used
  as trait objects, and native `async fn` in traits is not dyn-compatible.
- `with_added_extension` for `.part` files — would change the on-disk naming.
- Declaring an MSRV (`rust-version`).
