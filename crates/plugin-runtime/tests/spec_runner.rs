//! Plugin spec-file runner integration tests.
//!
//! Covers Phase 2.4 of `docs/specs/integration-tests.md`: wire the
//! existing `plugin.spec.ts` files into CI by executing them through
//! the runtime's in-engine test harness. Network-dependent tests in the
//! YouTube spec call `skip()` automatically when YouTube is unreachable,
//! so this suite works offline in CI.

use std::path::PathBuf;

use amigo_plugin_runtime::loader::PluginLoader;
use amigo_plugin_runtime::sandbox::SandboxLimits;

fn shipped_plugins_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/plugin-runtime should be two levels below repo root")
        .join("plugins")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_generic_http_plugin_spec_passes() {
    let loader =
        PluginLoader::new(shipped_plugins_dir(), SandboxLimits::default()).expect("loader init");
    loader.discover().await.expect("discover");

    let results = loader
        .run_spec("generic-http")
        .await
        .expect("run_spec for generic-http");

    assert_eq!(
        results.failed,
        0,
        "generic-http spec failures: {:#?}",
        results
            .results
            .iter()
            .filter(|r| !r.passed && !r.skipped)
            .collect::<Vec<_>>()
    );
    assert!(
        results.passed > 0,
        "generic-http spec should run at least one assertion, got {results:?}"
    );
}

/// Offline-stable subset of the YouTube spec: tests that don't touch
/// the network and must always pass.
const YOUTUBE_OFFLINE_TESTS: &[&str] = &[
    "has required metadata",
    "urlPattern matches youtube URLs",
    "has resolve function",
    "has checkOnline function",
];

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_youtube_plugin_spec_passes() {
    let loader =
        PluginLoader::new(shipped_plugins_dir(), SandboxLimits::default()).expect("loader init");
    loader.discover().await.expect("discover");

    let results = loader
        .run_spec("youtube")
        .await
        .expect("run_spec for youtube");

    let strict = std::env::var("AMIGO_NETWORK_TESTS").is_ok_and(|v| v == "1");

    // Offline-stable tests must always pass — even without internet, even
    // if YouTube changes a JSON field downstream.
    for name in YOUTUBE_OFFLINE_TESTS {
        let r = results
            .results
            .iter()
            .find(|r| &r.name.as_str() == name)
            .unwrap_or_else(|| panic!("offline test {name:?} not found in {:?}", results.results));
        assert!(
            r.passed,
            "offline-stable test {name:?} must pass, got {r:?}"
        );
    }

    // Network-dependent tests are only enforced when AMIGO_NETWORK_TESTS=1.
    // Otherwise YouTube reachability / API drift may flake the suite; the
    // spec is still executed so flakes surface in CI logs.
    if strict {
        assert_eq!(
            results.failed,
            0,
            "AMIGO_NETWORK_TESTS=1 set but failures: {:#?}",
            results
                .results
                .iter()
                .filter(|r| !r.passed && !r.skipped)
                .collect::<Vec<_>>()
        );
    }
}
