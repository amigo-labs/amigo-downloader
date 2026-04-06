fn main() {
    // Only run Tauri codegen when the `tauri` feature is active.
    // The feature is enabled automatically by `cargo tauri build`.
    #[cfg(feature = "tauri")]
    tauri_build::main();
}
