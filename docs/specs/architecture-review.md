# Architecture Review: amigo-downloader

**Date**: 2026-03-29
**Scope**: Full architecture evaluation of crate structure, core engine, plugin system, web-ui, and deployment

---

## Overall Verdict: 6.5/10 — Solid Foundation, No Major Rewrite Needed

The architecture is fundamentally sound. Crate separation is clean, the async model is appropriate, and the feature set is ambitious for the codebase size. The issues are evolutionary debt, not structural failures. A focused effort on 3-4 high-priority items would meaningfully improve reliability and extensibility.

---

## Ratings by Area

| Area | Score | Assessment |
|---|---|---|
| Crate structure & modularity | **8/10** | Clean 5-crate split, no circular dependencies |
| Coordinator / orchestration | **5/10** | Functional but monolithic, conflates too many concerns |
| Storage layer | **6/10** | WAL mode correct, `Arc<Mutex<Connection>>` adequate for current scale |
| Protocol system | **5/10** | Works but hardcoded enum prevents extension without lib changes |
| Bandwidth management | **3/10** | Token bucket implemented but NOT integrated into download loops — dead code |
| Chunk/resume support | **3/10** | Schema + types exist but coordinator ignores chunk persistence |
| Error handling | **4/10** | `Error::Other(String)` catch-all loses context |
| Retry logic | **6/10** | Works but inlined in coordinator, no jitter despite doc claim |
| Post-processing | **7/10** | Rust crates for RAR/ZIP/7z, only tar/par2 shell out, no injection risk |
| Plugin runtime | **8/10** | Well-designed sandbox limits, typed errors, clean separation |
| Event system | **6/10** | Broadcast channel functional, rigid enum but adequate |
| Testing | **2/10** | Only storage/config/bandwidth/postprocess have unit tests. Zero integration tests |
| Web UI | **7/10** | Svelte 5, typed API, PWA, well-structured |
| API design | **7/10** | Clean REST routes, versioned (`/api/v1/`) |
| Configuration | **8/10** | TOML + serde defaults, auto-discovery, tested |

---

## What Should NOT Change

1. **Crate structure** (core, server, cli, extractors, plugin-runtime) — well-chosen, one-directional deps
2. **Plugin sandbox design** — 30s timeout, 64MB RAM, 20 HTTP requests per invocation
3. **TOML configuration** — serde-based with defaults, feature flags pattern
4. **Post-processing** — Rust crates for extraction (unrar, zip, sevenz_rust), `Command::new` with explicit args for tar/par2
5. **Web-UI structure** — Svelte 5, stores, typed API client, PWA
6. **REST API versioning** — `/api/v1/` prefix throughout
7. **Broadcast channel for events** — `tokio::sync::broadcast` is the right choice for fan-out

---

## Prioritized Improvements

### HIGH IMPACT

#### H1. Integrate Bandwidth Limiting (effort: small, impact: high)

The `_bandwidth` parameter in `coordinator.rs` is prefixed with underscore — explicitly unused. The `BandwidthLimiter::acquire()` method works correctly (verified in tests). Fix: call `bandwidth.acquire(chunk.len())` inside the download loops in `http.rs`.

**Files**: `crates/core/src/protocol/http.rs`, `crates/core/src/coordinator.rs`

#### H2. Persist Chunk State for Resume (effort: medium, impact: high)

The `chunks` table exists in the DB schema, `ChunkPlan`/`Chunk` structs exist in `chunk.rs`, but `download_chunked` creates ephemeral temp files without recording state to the database. On resume, downloads restart from scratch.

Fix: persist `ChunkPlan` to chunks table before spawning tasks, update `bytes_downloaded` per chunk periodically, on resume check chunks table and skip completed chunks.

**Files**: `crates/core/src/storage.rs`, `crates/core/src/protocol/http.rs`, `crates/core/src/chunk.rs`

#### H3. Add Integration Tests for Core Flows (effort: medium, impact: high)

The `tests/integration/` and `tests/plugins/` directories are empty. At minimum:

1. Coordinator: add -> start -> complete lifecycle (mock HTTP server)
2. Coordinator: add -> fail -> retry -> exhaust retries
3. Coordinator: pause/resume/cancel state transitions
4. Storage: chunk persistence roundtrip
5. Plugin loader: load, resolve, timeout enforcement

`Storage::open_memory()` already exists for testing.

**Files**: `tests/integration/`, `Cargo.toml` (dev-deps: wiremock or mockito)

### MEDIUM IMPACT

#### M1. Extract Protocol Trait (effort: medium, impact: medium)

Currently `start_download` dispatches on string `protocol == "usenet"`. Adding a new protocol requires changes in 3+ places. Extract a `ProtocolHandler` trait with `detect()` + `download()` methods and a registry in the coordinator.

**Files**: `crates/core/src/protocol/mod.rs`, `crates/core/src/coordinator.rs`

#### M2. Add Error Context (effort: small, impact: medium)

`Error::Other(String)` is used 20+ times as a catch-all. Add structured variants:
- `Error::ProtocolError { protocol, source }`
- `Error::PostProcessError { path, tool, source }`
- `Error::PluginError { plugin_id, source }`

**File**: `crates/core/src/lib.rs`

#### M3. Extract Retry as Reusable Utility (effort: small, impact: medium)

100+ lines of inline retry logic in coordinator. Extract a generic `retry_with_policy` function and add real jitter (currently missing despite doc claims).

**File**: `crates/core/src/retry.rs`

#### M4. Clarify Extractors vs Plugins Boundary (effort: small, impact: medium)

YouTube exists as both a built-in extractor AND a plugin. Document clear policy:
- **Extractors**: built-in, for sites needing complex logic (cipher challenges) where JS sandbox overhead is unacceptable
- **Plugins**: for everything else, especially sites that change frequently and need OTA updates

### LOW IMPACT

- **L1.** Formalize state machine transitions — `can_transition_to()` guard method
- **L2.** Move DB to `spawn_blocking` or connection pool — only if scale becomes an issue
- **L3.** Event system extensibility — revisit only if variants exceed 20

---

## Recommended Implementation Order

| Phase | Items | Effort |
|---|---|---|
| Phase 1 (quick wins) | H1 bandwidth integration, M3 retry extraction | 1-2 days |
| Phase 2 (reliability) | H2 chunk persistence, H3 integration tests | 3-5 days |
| Phase 3 (extensibility) | M1 protocol trait, M2 error context, M4 boundary docs | 3-4 days |
| Phase 4 (polish) | L1-L3 | Only if needed |

---

## Critical Files

| File | LOC | Role |
|---|---|---|
| `crates/core/src/coordinator.rs` | 666 | Central orchestrator, main refactoring target |
| `crates/core/src/protocol/http.rs` | 381 | HTTP downloads, bandwidth + chunk integration |
| `crates/core/src/storage.rs` | 644 | SQLite abstraction, chunk CRUD needed |
| `crates/core/src/retry.rs` | 29 | Retry utility, needs extraction + jitter |
| `crates/core/src/lib.rs` | 42 | Error enum, needs structured variants |
| `crates/core/src/protocol/mod.rs` | 16 | Protocol trait target |
