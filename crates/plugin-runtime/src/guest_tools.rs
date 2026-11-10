//! What building the plugin guest needs, and how to say so when it is
//! missing. Used by `build.rs`; compiled into the crate only for its test.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Rust target the guest is built for.
pub const GUEST_TARGET: &str = "wasm32-unknown-unknown";

/// Environment variable that names the C compiler for the guest target; the
/// `cc` crate reads it too.
pub const CC_ENV: &str = "CC_wasm32_unknown_unknown";

const INSTALL_CLANG: &str = "Install it with:\n  \
    Linux:   sudo apt-get install clang lld\n  \
    macOS:   brew install llvm   (Apple's clang has no WebAssembly target), then put \
    $(brew --prefix llvm)/bin first in PATH or set CC_wasm32_unknown_unknown\n  \
    Windows: choco install llvm";

/// Find `name` (or `name.exe`) in a `PATH`-style list.
pub fn find_in_path(name: &str, search_path: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(search_path)
        .flat_map(|dir| [dir.join(name), dir.join(format!("{name}.exe"))])
        .find(|candidate| candidate.is_file())
}

/// Check the tools the guest build needs. `search_path` is the `PATH` to
/// look for `clang` in, `cc_override` the value of [`CC_ENV`] if set, and
/// `target_libdir` where the Rust standard library for [`GUEST_TARGET`]
/// would be installed.
pub fn check_guest_tools(
    search_path: &OsStr,
    cc_override: Option<&OsStr>,
    target_libdir: Option<&Path>,
) -> Result<(), String> {
    let mut missing = Vec::new();
    if cc_override.is_none() && find_in_path("clang", search_path).is_none() {
        missing.push(format!(
            "`clang` with a WebAssembly target was not found in PATH. {INSTALL_CLANG}"
        ));
    }
    if !target_libdir.is_some_and(Path::is_dir) {
        missing.push(format!(
            "the Rust target `{GUEST_TARGET}` is not installed. Install it with:\n  \
             rustup target add {GUEST_TARGET}"
        ));
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Cannot build the plugin guest (QuickJS as WebAssembly, crates/plugin-guest):\n- {}",
            missing.join("\n- ")
        ))
    }
}

/// Message for a `clang` that is installed but cannot target WebAssembly.
pub fn clang_without_wasm(clang: &Path) -> String {
    format!(
        "Cannot build the plugin guest: `{}` has no WebAssembly target. {INSTALL_CLANG}",
        clang.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_tools_are_named_with_install_commands() {
        let err = check_guest_tools(OsStr::new(""), None, None).unwrap_err();
        for needle in [
            "clang",
            "apt-get install clang",
            "brew install llvm",
            "choco install llvm",
            "rustup target add wasm32-unknown-unknown",
        ] {
            assert!(err.contains(needle), "message lacks `{needle}`:\n{err}");
        }
    }

    #[test]
    fn present_tools_pass() {
        let dir = std::env::temp_dir();
        assert!(
            check_guest_tools(OsStr::new(""), Some(OsStr::new("my-clang")), Some(&dir)).is_ok()
        );
    }

    #[test]
    fn clang_without_a_wasm_target_names_the_fix() {
        let message = clang_without_wasm(Path::new("/usr/bin/clang"));
        assert!(message.contains("/usr/bin/clang"), "{message}");
        assert!(message.contains("brew install llvm"), "{message}");
        assert!(message.contains(CC_ENV), "{message}");
    }
}
