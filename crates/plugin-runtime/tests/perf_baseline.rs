//! Performance baseline for the plugin runtime (issues #79, #83, #85).
//!
//! Ignored by default — run explicitly and read the printed numbers:
//!
//! ```sh
//! KRAKE_REGISTRY_PUBKEY_HEX=$(printf "de%.0s" {1..32}) \
//!   cargo test -p krake-plugin-runtime --release --test perf_baseline -- --ignored --nocapture
//! ```
//!
//! Measures, against the shipped `plugins/` tree:
//! - `discover()` cold (transpile + compile every plugin) vs warm (bytecode
//!   cache hit);
//! - `resolve()` of `generic-http` against a local mock page, i.e. the JS +
//!   host-API cost of a resolve with the network latency factored out. Since
//!   #79 this includes the IPC round trips to the `krake-plugin-host` child;
//!   the numbers before and after are recorded in
//!   `docs/specs/plugin-sandbox-hardening.md` (SH-7a).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use krake_plugin_runtime::bytecode_cache::BytecodeCache;
use krake_plugin_runtime::loader::PluginLoader;
use krake_plugin_runtime::sandbox::SandboxLimits;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn shipped_plugins_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/plugin-runtime should be two levels below repo root")
        .join("plugins")
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "performance baseline; run with --ignored --nocapture"]
async fn plugin_load_and_resolve_baseline() {
    const RUNS: usize = 5;
    let cache_root = tempfile::tempdir().unwrap();

    let mut cold = Vec::new();
    let mut warm = Vec::new();
    let mut count = 0;
    for run in 0..RUNS {
        let cache_dir = cache_root.path().join(format!("run-{run}"));
        for (phase, samples) in [("cold", &mut cold), ("warm", &mut warm)] {
            // A fresh process-wide transpile cache cannot be simulated, so the
            // cold numbers include the in-memory SWC cache from earlier runs
            // only on runs > 0; the first-run figure is printed separately.
            let loader = PluginLoader::new(shipped_plugins_dir(), SandboxLimits::default())
                .unwrap()
                .with_bytecode_cache(BytecodeCache::open(&cache_dir, 64).unwrap());
            let start = Instant::now();
            count = loader.discover().await.unwrap().len();
            let elapsed = start.elapsed();
            if run == 0 && phase == "cold" {
                println!("discover() first cold start (SWC + compile): {elapsed:?}");
            }
            samples.push(elapsed);
        }
    }
    println!("plugins loaded: {count}");
    let (cold, warm) = (median(cold), median(warm));
    println!("discover() cold, median of {RUNS}: {cold:?}");
    println!("discover() warm (bytecode cache), median of {RUNS}: {warm:?}");
    // Budgets of docs/specs/plugin-wasm-sandbox.md AC-07; they hold for an
    // optimized build only.
    let budgets = !cfg!(debug_assertions);
    if budgets {
        assert!(cold < Duration::from_millis(60), "cold: {cold:?}");
        assert!(warm < Duration::from_millis(40), "warm: {warm:?}");
    }

    let mock = MockServer::start().await;
    let page = r#"<html><head><title>Bench</title></head><body>
        <a href="/files/archive.zip">Download archive.zip</a></body></html>"#;
    Mock::given(method("HEAD"))
        .and(path("/page"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/html"))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/page"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/html")
                .set_body_string(page),
        )
        .mount(&mock)
        .await;
    Mock::given(method("HEAD"))
        .and(path("/files/archive.zip"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/zip")
                .insert_header("content-length", "1024")
                .insert_header("accept-ranges", "bytes"),
        )
        .mount(&mock)
        .await;

    let limits = SandboxLimits {
        allow_private_network: true,
        ..SandboxLimits::default()
    };
    let loader = PluginLoader::new(shipped_plugins_dir(), limits).unwrap();
    loader.discover().await.unwrap();
    let url = format!("{}/page", mock.uri());

    let mut samples = Vec::new();
    for _ in 0..50 {
        let start = Instant::now();
        let pkg = loader.resolve("generic-http", &url).await.unwrap();
        samples.push(start.elapsed());
        assert_eq!(pkg.downloads.len(), 1);
    }
    let resolve = median(samples);
    println!("generic-http resolve() against local mock (3 HTTP calls), median of 50: {resolve:?}");
    if budgets {
        assert!(resolve < Duration::from_millis(5), "resolve: {resolve:?}");
    }
}

/// Start-up cost of the Wasm executor (docs/specs/plugin-wasm-guest.md,
/// AC-13): making the embedded guest ready, and creating one plugin instance.
#[test]
#[ignore = "performance baseline; run with --ignored --nocapture"]
fn wasm_executor_startup_baseline() {
    use krake_plugin_runtime::engine::EngineConfig;
    use krake_plugin_runtime::host_fns::{HostArgs, HostDispatch, HostResult};
    use krake_plugin_runtime::wasm::WasmEngine;

    struct Nothing;
    impl HostDispatch for Nothing {
        fn call(&self, _name: &str, _args: HostArgs) -> HostResult {
            Ok(None)
        }
    }

    const RUNS: usize = 5;
    let mut create_engine = Vec::new();
    let mut create_instance = Vec::new();
    for _ in 0..RUNS {
        let start = Instant::now();
        let engine = WasmEngine::new().unwrap();
        create_engine.push(start.elapsed());

        let start = Instant::now();
        let context = engine
            .create_context(&EngineConfig::default(), std::sync::Arc::new(Nothing))
            .unwrap();
        create_instance.push(start.elapsed());
        assert_eq!(context.eval_js("String(1 + 1)").unwrap(), "2");
    }
    let (engine, instance) = (median(create_engine), median(create_instance));
    println!("wasm executor: create engine {engine:?}, create instance {instance:?}");
    assert!(engine < Duration::from_millis(50), "engine: {engine:?}");
    assert!(
        instance < Duration::from_millis(5),
        "instance: {instance:?}"
    );
}
