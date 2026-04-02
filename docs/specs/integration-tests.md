# Integration Tests Spec

**Date**: 2026-04-02
**Status**: Draft
**Priority**: High (architecture review rated Testing at 2/10)
**Effort**: Medium (3-5 days)

---

## Problem Statement

The project has 14 coordinator-level tests (`crates/core/tests/coordinator.rs`) and scattered unit tests, but zero integration tests. The `tests/integration/` and `tests/plugins/` directories contain only `.gitkeep` files. The architecture review (2026-03-29) flagged this as item H3. Specific gaps:

1. **Empty test directories** — `tests/integration/.gitkeep` and `tests/plugins/.gitkeep` are placeholders with no test code.
2. **No server API endpoint tests** — The Axum server exposes 40+ REST endpoints across `api.rs`, `update_api.rs`, `clicknload.rs`, `nzbget_api.rs`, `feedback.rs`, and `ws.rs`. None have integration tests exercising request/response through the HTTP layer.
3. **No plugin runtime integration tests** — Plugin loading, TypeScript transpilation via SWC, sandbox execution, timeout enforcement, and host API calls are untested end-to-end.
4. **No dedicated storage layer tests** — The coordinator tests exercise some storage paths (insert, get, chunk roundtrip), but edge cases like concurrent access, migration correctness, and error recovery are not covered.
5. **Plugin spec tests not in CI** — `plugins/extractors/youtube/plugin.spec.ts` (100 lines, 8 tests) and `plugins/hosters/generic-http/plugin.spec.ts` (25 lines, 4 tests) exist but are never executed in CI pipelines.

---

## Goals

- Establish integration test infrastructure that is easy to extend.
- Cover the most critical paths first: API endpoints, plugin lifecycle, storage edge cases.
- Integrate existing plugin spec tests into CI.
- Keep tests fast: no real network calls (except opt-in plugin spec tests), in-memory storage where possible.

## Non-Goals

- 100% endpoint coverage in the first pass.
- Performance/load testing.
- UI (Svelte) integration tests.
- End-to-end tests requiring a running browser.

---

## Phase 1: Test Infrastructure & Server API (Priority: Highest)

### 1.1 Test Infrastructure Setup

Add shared test utilities that all integration tests can reuse.

**What to build:**
- A `test_helpers` module (in `tests/integration/helpers/mod.rs` or a `dev-dependencies` test-support crate) providing:
  - `spawn_test_server() -> (SocketAddr, AppState)` — starts the Axum server on a random port with in-memory SQLite (`Storage::open_memory()`), a no-op plugin loader, and returns the bound address.
  - `test_client(addr) -> reqwest::Client` — pre-configured HTTP client pointing at the test server.
  - `cleanup()` — graceful shutdown helper.
- Dev-dependencies in workspace `Cargo.toml`: `reqwest` (with `json` feature), `tokio-test`, `wiremock` or `mockito` for HTTP mocking.

**Acceptance Criteria:**
- [ ] `tests/integration/helpers/` module compiles and is importable from any test file in `tests/integration/`.
- [ ] `spawn_test_server()` returns a live server that responds to `GET /api/v1/status` with 200.
- [ ] Server uses in-memory storage (no disk I/O during tests).
- [ ] Tests can run in parallel without port conflicts.

### 1.2 Server API Endpoint Tests

Test the REST API through actual HTTP requests against the Axum router.

**File:** `tests/integration/api_tests.rs`

**Endpoints to cover (priority order):**

| Group | Endpoints | Key assertions |
|---|---|---|
| Status | `GET /api/v1/status`, `GET /api/v1/stats` | Returns 200, JSON body has expected fields |
| Downloads CRUD | `POST /api/v1/downloads`, `GET /api/v1/downloads`, `GET /api/v1/downloads/{id}`, `PATCH /api/v1/downloads/{id}`, `DELETE /api/v1/downloads/{id}` | Create returns 201 with ID; list returns array; get returns correct entry; patch changes status; delete returns 204 and entry is gone |
| Batch | `POST /api/v1/downloads/batch` | Accepts array of URLs, returns array of IDs |
| Queue | `GET /api/v1/queue`, `PATCH /api/v1/queue/reorder` | Queue reflects added downloads; reorder changes priority order |
| History | `GET /api/v1/history`, `DELETE /api/v1/history` | History is empty initially; delete clears entries |
| Config | `GET /api/v1/config`, `PUT /api/v1/config` | Get returns current config; put updates and persists |
| Plugins | `GET /api/v1/plugins`, `POST /api/v1/plugins/suggest` | List returns loaded plugins; suggest matches URL to plugin |
| Usenet | `GET/POST/DELETE /api/v1/usenet/servers` | CRUD lifecycle for usenet server config |
| RSS | `GET/POST/DELETE /api/v1/rss` | CRUD lifecycle for RSS feeds |
| Captcha | `GET /api/v1/captcha/pending` | Returns empty array when no captchas pending |
| Webhooks | `GET/POST/DELETE /api/v1/webhooks`, `POST /api/v1/webhooks/{id}/test` | Full CRUD lifecycle; test fires a webhook |
| Click'n'Load | `GET /jdcheck.js`, `POST /flash/add` | jdcheck returns JS; flash/add accepts links |
| Error cases | `GET /api/v1/downloads/nonexistent`, `PATCH` with invalid body | Returns 404; returns 400/422 |

**Acceptance Criteria:**
- [ ] At least 20 test functions covering the endpoint groups above.
- [ ] Each test starts a fresh server (or uses shared state with unique data).
- [ ] Tests validate HTTP status codes AND response body structure.
- [ ] Error cases return appropriate 4xx status codes, not 500.
- [ ] All tests pass with `cargo test --test api_tests`.

### 1.3 WebSocket Integration Tests

**File:** `tests/integration/ws_tests.rs`

**Acceptance Criteria:**
- [ ] Test connects to `ws://addr/api/v1/ws` and receives events.
- [ ] Adding a download via REST triggers an `added` event on the WebSocket.
- [ ] Pausing a download triggers a `status_changed` event.
- [ ] Multiple concurrent WebSocket connections each receive events.

---

## Phase 2: Plugin Runtime Integration Tests (Priority: High)

### 2.1 Plugin Loading & Transpilation

Test the full plugin lifecycle: discover `.ts` file, transpile via SWC, load into QuickJS, execute.

**File:** `tests/integration/plugin_runtime_tests.rs`

**Acceptance Criteria:**
- [ ] Load the `generic-http` plugin from `plugins/hosters/generic-http/plugin.ts` — verify `plugin_id()`, `plugin_name()`, `url_pattern()` return expected values.
- [ ] Load the `youtube` plugin from `plugins/extractors/youtube/plugin.ts` — verify metadata.
- [ ] A syntactically invalid `.ts` file produces a clear transpilation error (not a panic).
- [ ] A plugin missing required exports (`plugin_id`, `resolve`) produces a clear load error.
- [ ] TypeScript-specific syntax (interfaces, type annotations, enums) transpiles correctly.

### 2.2 Sandbox Enforcement

**File:** `tests/integration/plugin_sandbox_tests.rs`

**Acceptance Criteria:**
- [ ] A plugin exceeding the 30-second timeout is terminated and returns a timeout error.
- [ ] A plugin exceeding 64MB memory allocation is terminated with an OOM error.
- [ ] A plugin attempting more than 20 HTTP requests in a single invocation is denied.
- [ ] A plugin cannot access the filesystem directly (no `Deno.readFile`, `require('fs')`, etc.).
- [ ] A plugin cannot spawn processes.

### 2.3 Host API Integration

**File:** `tests/integration/plugin_host_api_tests.rs`

**Acceptance Criteria:**
- [ ] `http_get` / `http_post` through the host API reach a mock server (wiremock) and return correct responses.
- [ ] `storage_get` / `storage_set` persist across calls within the same plugin invocation.
- [ ] `regex_match`, `html_select`, `base64_encode/decode`, `json_parse` return correct results.
- [ ] `set_wait`, `set_filename`, `set_filesize` correctly propagate values to the host.

### 2.4 Automate Existing Plugin Spec Tests

Wire the existing `plugin.spec.ts` files into CI.

**Acceptance Criteria:**
- [ ] A Rust integration test (or CI script) invokes the plugin test runner for `plugins/extractors/youtube/plugin.spec.ts`.
- [ ] A Rust integration test invokes the plugin test runner for `plugins/hosters/generic-http/plugin.spec.ts`.
- [ ] Tests that require network access (YouTube reachability) are skipped in CI by default, runnable with an opt-in flag (e.g., `AMIGO_NETWORK_TESTS=1`).
- [ ] Plugin spec test results appear in CI output with pass/fail per test case.
- [ ] CI pipeline runs plugin spec tests on every PR.

---

## Phase 3: Storage Layer Tests (Priority: Medium)

### 3.1 Storage Edge Cases

The coordinator tests cover the happy path. These tests target edge cases.

**File:** `tests/integration/storage_tests.rs`

**Acceptance Criteria:**
- [ ] Insert a download, close the storage, reopen (on-disk SQLite in a tempdir), verify data persists.
- [ ] Insert 1000 downloads, verify list pagination works correctly.
- [ ] Update a download that does not exist — returns an appropriate error, not a silent success.
- [ ] Delete a download with associated chunks — chunks are also deleted (cascade).
- [ ] Concurrent writes from multiple tasks do not cause "database is locked" errors (WAL mode validation).
- [ ] History: complete a download, verify it appears in history; delete history, verify it is cleared.
- [ ] Usenet server CRUD: add, list, delete server configs through storage layer.
- [ ] RSS feed CRUD: add, list, delete feeds through storage layer.
- [ ] Chunk state edge cases: save chunks for a download, update one chunk to "completed", verify partial resume state is correct.

### 3.2 Migration Tests

**Acceptance Criteria:**
- [ ] Opening a fresh database creates all expected tables (`downloads`, `chunks`, `config`, `history`, `usenet_servers`, `rss_feeds`, `webhooks`).
- [ ] Schema version is tracked and queryable.

---

## Phase 4: Core Lifecycle Integration Tests (Priority: Medium)

### 4.1 Download Lifecycle with Mock HTTP

Use wiremock to simulate an HTTP server serving a file.

**File:** `tests/integration/download_lifecycle_tests.rs`

**Acceptance Criteria:**
- [ ] Add a URL pointing at wiremock, start the download, verify it completes and the file exists on disk.
- [ ] Add a URL, start download, wiremock returns 503 — verify retry logic kicks in and status reflects retrying.
- [ ] Add a URL, start download, cancel mid-download — verify temp files are cleaned up.
- [ ] Add a URL pointing at wiremock with `Accept-Ranges: bytes` — verify chunked download splits into multiple requests.
- [ ] Pause a downloading file, resume — verify it continues from the last byte (Range header sent).

### 4.2 Post-Processing Pipeline

**Acceptance Criteria:**
- [ ] Download a `.zip` file (from wiremock), verify auto-extraction runs and extracted files exist.
- [ ] Download a non-archive file — verify post-processing completes without error (no-op).

---

## Implementation Notes

### Test Infrastructure Requirements

1. **Dev-dependencies to add** (workspace `Cargo.toml`):
   - `wiremock = "0.6"` — HTTP mock server for download and plugin host API tests
   - `tokio-tungstenite` — WebSocket client for WS tests
   - `tempfile` — temporary directories for on-disk storage tests
   - `reqwest` (already a dependency, ensure `json` feature in dev-deps)

2. **Test file structure**:
   ```
   tests/
   ├── integration/
   │   ├── helpers/
   │   │   ├── mod.rs           # spawn_test_server, test_client, etc.
   │   │   └── fixtures.rs      # Test data constants (URLs, sample NZBs, etc.)
   │   ├── api_tests.rs
   │   ├── ws_tests.rs
   │   ├── plugin_runtime_tests.rs
   │   ├── plugin_sandbox_tests.rs
   │   ├── plugin_host_api_tests.rs
   │   ├── storage_tests.rs
   │   └── download_lifecycle_tests.rs
   └── plugins/
       └── run_specs.rs         # Harness to run plugin.spec.ts files via QuickJS
   ```

3. **CI integration**: Each test file is a separate `[[test]]` binary in `Cargo.toml` so they run in parallel. Tests requiring network access are gated behind `#[cfg(feature = "network-tests")]` or an environment variable check.

4. **Test execution time target**: All non-network tests should complete in under 30 seconds on CI. Timeout-enforcement tests (sandbox 30s) should use a reduced timeout for testing (e.g., 1 second).

### Patterns from Existing Tests

The coordinator tests establish these patterns to follow:
- Use `Storage::open_memory()` for in-memory SQLite (no disk I/O).
- Use `#[tokio::test]` for async tests.
- Set `config.max_concurrent_downloads = 0` to prevent auto-start when testing state management.
- Assert on both status codes/values and structural correctness (field presence).

### Risk Considerations

- **Sandbox timeout tests** may be flaky on slow CI runners. Use generous margins (e.g., assert timeout occurs within 1-5 seconds, not exactly at 1 second).
- **Plugin spec tests with network** (YouTube) should always be opt-in and use `skip()` when the network is unavailable, as the existing spec files already do.
- **Port conflicts** — use `0` for port binding (OS-assigned) to avoid conflicts in parallel test runs.

---

## Acceptance Criteria Summary

| Phase | Tests | Minimum count |
|---|---|---|
| Phase 1 | Server API + WebSocket | 25 tests |
| Phase 2 | Plugin runtime + sandbox + host API + spec runner | 20 tests |
| Phase 3 | Storage edge cases + migrations | 12 tests |
| Phase 4 | Download lifecycle + post-processing | 7 tests |
| **Total** | | **64 tests** |

All tests must:
- Pass with `cargo test` from the workspace root.
- Not require network access (except opt-in plugin spec tests).
- Not leave temp files or zombie processes.
- Complete in under 30 seconds (excluding network tests).
