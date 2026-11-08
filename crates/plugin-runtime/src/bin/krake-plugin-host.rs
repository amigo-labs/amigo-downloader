//! `krake-plugin-host` — runs plugin JavaScript outside the daemon.
//!
//! Started by `krake-server` / `krake` with piped stdin/stdout; not meant
//! to be run by hand. See `krake_plugin_runtime::plugin_host`.

fn main() {
    std::process::exit(krake_plugin_runtime::plugin_host::run());
}
