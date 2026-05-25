# Integration Tests

Integration tests for this workspace live **next to the crate they
exercise**, not in this top-level directory. The split was originally
proposed in `docs/specs/integration-tests.md` but per-crate `tests/`
directories were already established by the time the spec was written,
so we kept them.

Where to find tests by area:

| Area | Location | Notable files |
|---|---|---|
| REST API + auth + WebSocket | `crates/server/tests/` | `api_tests.rs`, `auth_flow_tests.rs`, `ws_tests.rs` |
| Coordinator + storage + download lifecycle | `crates/core/tests/` | `coordinator.rs`, `storage_tests.rs`, `download_lifecycle_tests.rs` |
| Plugin loader, sandbox, host-API, spec runner | `crates/plugin-runtime/tests/` | `runtime_tests.rs`, `sandbox_tests.rs`, `host_api_tests.rs`, `spec_runner.rs` |

Run everything with `cargo test --workspace`.

Plugin spec files (`plugins/**/plugin.spec.ts`) are executed through the
QuickJS-backed runner in `crates/plugin-runtime/tests/spec_runner.rs`.
Network-dependent assertions in those specs are gated behind
`AMIGO_NETWORK_TESTS=1`; without that variable they are still parsed and
executed, but failures are downgraded to warnings.
