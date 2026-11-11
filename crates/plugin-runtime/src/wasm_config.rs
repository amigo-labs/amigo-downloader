// Included by `build.rs` (which precompiles the guest module) and by
// `src/wasm.rs` (which loads it): a precompiled module only loads into an
// engine configured the same way.

/// Stack a guest call may use before it traps. QuickJS's own stack check is
/// off in the guest, so this is what stops unbounded recursion.
const GUEST_STACK_BYTES: usize = 512 * 1024;

fn guest_engine_config() -> wasmtime::Config {
    let mut config = wasmtime::Config::new();
    // Deadlines: a ticker advances the epoch, each call sets how many ticks
    // it may run.
    config.epoch_interruption(true);
    config.max_wasm_stack(GUEST_STACK_BYTES);
    config
}
