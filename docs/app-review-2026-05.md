# App Review — May 2026

Scope: a focused correctness + performance pass across `core`,
`plugin-runtime`/`extractors`, `server`, and `web-ui`. A broad automated sweep
produced ~40 candidate findings; each was validated against the actual code
before any change. This document records what was fixed, what was rejected as a
false positive (so it is not "re-discovered"), and what is deliberately
deferred.

## Fixed in this change

| # | Area | Type | Summary |
|---|------|------|---------|
| 1 | `core` | **Bug** | Downloads could not be cancelled while stuck in the retry loop. |
| 2 | `core` | Robustness | Speed calculation could divide by a zero interval (→ `inf`). |
| 3 | `plugin-runtime` | Perf | Plugin regex helpers recompiled the pattern on every call. |
| 4 | `plugin-runtime` | Correctness | base64 decoder silently accepted invalid characters. |
| 5 | `plugin-runtime` | Correctness | Plugin-VM scripts built via hand-rolled string escaping. |

### 1. Cancel during retry (main fix)

`Coordinator` carried cancellation over a `oneshot` channel. The receiver was
consumed by the first download attempt; every subsequent retry got a throwaway
receiver that never fired, so a download repeatedly failing-and-retrying could
not be cancelled or paused (the original code even acknowledged this as an
"audit #13 follow-up"). Cancel detection also relied on a brittle
`e.to_string().contains("cancelled")` substring match.

Fix:
- New `Error::Cancelled` variant; the retry loop matches it explicitly and
  aborts instead of treating it as a transient failure.
- `ProtocolBackend::download` now takes a `tokio::sync::watch::Receiver<bool>`
  (no new dependency — `watch` was already used) instead of a one-shot. A
  shared `wait_for_cancel` helper resolves when the flag flips and stays pending
  if the sender is dropped. Each retry clones a fresh receiver from the same
  sender, so a single `cancel`/`pause` is observed across all attempts.
- HTTP / HLS / DASH backends and the CLI direct-download path updated
  accordingly.

### 2. Speed division guard

`bytes / elapsed.as_secs_f64()` is now routed through a `bytes_per_sec` helper
that returns `0` for a zero/degenerate interval, preventing an `inf`/`NaN` cast
to a nonsensical `u64`.

### 3. Regex compile cache

`compile_plugin_regex` is the single chokepoint for all `regexMatch/MatchAll/
Replace/Test/Split` host functions. It now caches compiled `regex::Regex`
values by pattern in a bounded process-wide map (cap 256, cleared wholesale on
overflow). `Regex` is internally reference-counted, so cache hits are cheap and
turn hot-loop regex usage from "recompile every call" into a map lookup. Input
size limits are still enforced before the lookup.

### 4. base64 decoder hardening

`base64_decode_bytes` built a lookup table that mapped every non-alphabet byte
to `0` (= `A`), so malformed input decoded to silent garbage. It now uses a
`0xFF` sentinel and returns `invalid base64 character` for any byte that is not
an alphabet symbol or stripped whitespace/padding (newlines, CR, space, tab,
`=`).

### 5. JS interpolation via `serde_json`

`call_resolve` and `call_post_process` built the eval'd script by hand-escaping
quotes/backslashes. They now serialise the URL / context with
`serde_json::to_string`, which yields a correctly-escaped JS string literal and
removes a class of escaping edge cases.

## Rejected (validated false positives)

These were reported by the automated sweep but do not hold up against the code:

- **Chunk-index out-of-bounds panic** (`http.rs`): `chunk_progress` is sized
  exactly `num_chunks` and indexed by `0..num_chunks` — always in bounds.
- **`content_length.unwrap()` race** (`http.rs`): `head` is an immutable local;
  no concurrent mutation is possible between the `is_some_and` check and the
  `unwrap`.
- **Chunk-size off-by-one** (`http.rs`): the last chunk is special-cased to
  `total_size - 1`, so integer division leaves no gap.
- **NZB watch-folder path traversal** (`background.rs`): the watch directory is
  an admin-configured setting; choosing its location is by design, not an
  escalation.
- **Web-UI speed field mismatch**: `App.svelte` already maps
  `progress.speed_bytes_per_sec` onto the store's `speed` field.
- **player.js cache memory leak** (`n_challenge.rs`): the cache already enforces
  both a 12 h TTL and an 8-entry LRU cap (oldest evicted on insert).

## Deferred recommendations

Not addressed here to keep the change low-risk; worth a dedicated follow-up:

- **Storage off the async executor**: `Storage` holds a synchronous
  `rusqlite::Connection` behind an async mutex. Local SQLite queries are fast,
  but moving them onto `spawn_blocking` (or an async SQLite layer) would remove
  any chance of stalling the runtime under contention.
- **`Arc<Config>` instead of per-download clones**: `Coordinator` clones the
  full `Config` for each started download. Sharing an `Arc<Config>` (swapped via
  `RwLock`) would cut allocation churn under high concurrency.
- **SSRF DNS pinning**: plugin HTTP and webhook dispatch validate the resolved
  IP class but re-resolve at request time, leaving a theoretical DNS-rebinding
  window. Pinning the resolved IP between check and connect would close it. This
  is a deliberate, documented trade-off for now.
