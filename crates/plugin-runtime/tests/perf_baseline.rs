//! Performance baseline for the plugin runtime (issues #79, #83, #85).
//!
//! Ignored by default — run explicitly and read the printed numbers:
//!
//! ```sh
//! AMIGO_REGISTRY_PUBKEY_HEX=$(printf "de%.0s" {1..32}) \
//!   cargo test -p amigo-plugin-runtime --release --test perf_baseline -- --ignored --nocapture
//! ```
//!
//! Measures, against the shipped `plugins/` tree:
//! - `discover()` cold (transpile + compile every plugin) vs warm (bytecode
//!   cache hit);
//! - `resolve()` of `generic-http` against a local mock page, i.e. the
//!   in-process JS + host-API cost of a resolve with the network latency
//!   factored out. This is the baseline an out-of-process (#79) or Wasm
//!   (#85) runtime has to be compared against.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use amigo_plugin_runtime::bytecode_cache::BytecodeCache;
use amigo_plugin_runtime::loader::PluginLoader;
use amigo_plugin_runtime::sandbox::SandboxLimits;
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
    println!("discover() cold, median of {RUNS}: {:?}", median(cold));
    println!(
        "discover() warm (bytecode cache), median of {RUNS}: {:?}",
        median(warm)
    );

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
    println!(
        "generic-http resolve() against local mock (3 HTTP calls), median of 50: {:?}",
        median(samples)
    );
}
