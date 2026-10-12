//! `amigo-plugin-host` — runs plugin JavaScript outside the daemon.
//!
//! Started by `amigo-server` / `amigo-dl` with piped stdin/stdout; not meant
//! to be run by hand. See `amigo_plugin_runtime::plugin_host`.

fn main() {
    std::process::exit(amigo_plugin_runtime::plugin_host::run());
}
