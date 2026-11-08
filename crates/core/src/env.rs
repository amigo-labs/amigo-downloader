//! Environment variables under the `KRAKE_` prefix, with the pre-rename
//! `AMIGO_` names as a deprecated fallback.
//!
//! Every runtime variable is read through [`var`] / [`var_os`]: the new name
//! wins; if only the old `AMIGO_*` name is set, its value is used and a
//! deprecation warning is logged once per variable.

use std::collections::HashSet;
use std::ffi::OsString;
use std::sync::{LazyLock, Mutex};

const PREFIX: &str = "KRAKE_";
const LEGACY_PREFIX: &str = "AMIGO_";

/// Old variable names already warned about in this process.
static WARNED: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// The pre-rename name of `name` (`KRAKE_BIND` → `AMIGO_BIND`), if it has one.
pub fn legacy_name(name: &str) -> Option<String> {
    name.strip_prefix(PREFIX)
        .map(|rest| format!("{LEGACY_PREFIX}{rest}"))
}

/// Read the `KRAKE_*` variable `name`, falling back to its deprecated
/// `AMIGO_*` name. Mirrors [`std::env::var`]; the error is the one for the
/// new name.
pub fn var(name: &str) -> Result<String, std::env::VarError> {
    match std::env::var(name) {
        Err(std::env::VarError::NotPresent) => match legacy_name(name) {
            Some(old) => match std::env::var(&old) {
                Ok(value) => {
                    warn_deprecated(&old, name);
                    Ok(value)
                }
                Err(e) => Err(e),
            },
            None => Err(std::env::VarError::NotPresent),
        },
        other => other,
    }
}

/// [`var`] for values that need not be UTF-8. Mirrors [`std::env::var_os`].
pub fn var_os(name: &str) -> Option<OsString> {
    if let Some(value) = std::env::var_os(name) {
        return Some(value);
    }
    let old = legacy_name(name)?;
    let value = std::env::var_os(&old)?;
    warn_deprecated(&old, name);
    Some(value)
}

fn warn_deprecated(old: &str, new: &str) {
    let first = WARNED
        .lock()
        .map(|mut warned| warned.insert(old.to_string()))
        .unwrap_or(false);
    if first {
        tracing::warn!("{old} is deprecated and will be removed; set {new} instead");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each test uses its own variable names, so parallel tests don't race.
    fn set(name: &str, value: &str) {
        // SAFETY: the names are unique to one test; no other thread reads them.
        unsafe { std::env::set_var(name, value) }
    }

    #[test]
    fn legacy_name_maps_prefix() {
        assert_eq!(legacy_name("KRAKE_BIND").as_deref(), Some("AMIGO_BIND"));
        assert_eq!(legacy_name("HOME"), None);
    }

    #[test]
    fn var_prefers_new_name() {
        set("KRAKE_ENVTEST_BOTH", "new");
        set("AMIGO_ENVTEST_BOTH", "old");
        assert_eq!(var("KRAKE_ENVTEST_BOTH").as_deref(), Ok("new"));
        assert_eq!(var_os("KRAKE_ENVTEST_BOTH"), Some(OsString::from("new")));
    }

    #[test]
    fn var_falls_back_to_legacy_name() {
        set("AMIGO_ENVTEST_OLD", "old");
        assert_eq!(var("KRAKE_ENVTEST_OLD").as_deref(), Ok("old"));
        assert_eq!(var_os("KRAKE_ENVTEST_OLD"), Some(OsString::from("old")));
        assert!(
            WARNED
                .lock()
                .expect("warned set")
                .contains("AMIGO_ENVTEST_OLD")
        );
    }

    #[test]
    fn var_reports_missing() {
        assert_eq!(
            var("KRAKE_ENVTEST_MISSING"),
            Err(std::env::VarError::NotPresent)
        );
        assert_eq!(var_os("KRAKE_ENVTEST_MISSING"), None);
    }
}
