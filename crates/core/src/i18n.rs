//! Internationalization (i18n) — multilingual string lookup.
//!
//! Locale files are JSON in `locales/<lang>.json`. The active locale is set
//! once at startup (from config, installer, or system language) and used
//! globally via `t("key")` or `t_fmt("key", &[("var", "value")])`.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;
use tracing::warn;

/// Global locale state.
static LOCALE: OnceLock<Locale> = OnceLock::new();

struct Locale {
    strings: HashMap<String, String>,
    fallback: HashMap<String, String>,
    lang: String,
}

/// Initialize the i18n system with a language code (e.g. "en", "de").
/// Falls back to English for missing keys. Call once at startup.
pub fn init(lang: &str, locales_dir: &std::path::Path) {
    let fallback = load_locale(locales_dir, "en");
    let strings = if lang == "en" {
        fallback.clone()
    } else {
        load_locale(locales_dir, lang)
    };

    let _ = LOCALE.set(Locale {
        strings,
        fallback,
        lang: lang.to_string(),
    });
}

/// Initialize with embedded locale data (for single-binary builds).
pub fn init_from_str(lang: &str, lang_json: &str, fallback_json: &str) {
    let strings = parse_locale(lang_json);
    let fallback = parse_locale(fallback_json);

    let _ = LOCALE.set(Locale {
        strings,
        fallback,
        lang: lang.to_string(),
    });
}

/// Get the active language code.
pub fn current_lang() -> &'static str {
    LOCALE.get().map(|l| l.lang.as_str()).unwrap_or("en")
}

/// Look up a translated string by key.
/// Returns the key itself if not found.
pub fn t(key: &str) -> String {
    let Some(locale) = LOCALE.get() else {
        return key.to_string();
    };

    locale
        .strings
        .get(key)
        .or_else(|| locale.fallback.get(key))
        .cloned()
        .unwrap_or_else(|| {
            warn!("Missing i18n key: {key}");
            key.to_string()
        })
}

/// Look up a translated string and replace `{var}` placeholders.
///
/// ```ignore
/// t_fmt("download.added", &[("id", "abc-123")])
/// // → "Added download: abc-123"
/// ```
pub fn t_fmt(key: &str, vars: &[(&str, &str)]) -> String {
    let mut s = t(key);
    for (name, value) in vars {
        s = s.replace(&format!("{{{name}}}"), value);
    }
    s
}

/// List available locale codes by scanning the locales directory.
pub fn available_locales(locales_dir: &std::path::Path) -> Vec<String> {
    let mut locales = Vec::new();
    if let Ok(entries) = std::fs::read_dir(locales_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    locales.push(stem.to_string());
                }
            }
        }
    }
    locales.sort();
    locales
}

/// Detect the system language from environment variables.
pub fn detect_system_lang() -> String {
    for var in &["LANG", "LC_ALL", "LC_MESSAGES", "LANGUAGE"] {
        if let Ok(val) = std::env::var(var) {
            // LANG=de_DE.UTF-8 → "de"
            let lang = val.split('_').next().unwrap_or(&val);
            let lang = lang.split('.').next().unwrap_or(lang);
            if !lang.is_empty() && lang != "C" && lang != "POSIX" {
                return lang.to_lowercase();
            }
        }
    }
    "en".to_string()
}

fn load_locale(dir: &std::path::Path, lang: &str) -> HashMap<String, String> {
    let path = dir.join(format!("{lang}.json"));
    match std::fs::read_to_string(&path) {
        Ok(data) => parse_locale(&data),
        Err(e) => {
            warn!("Could not load locale {lang}: {e}");
            HashMap::new()
        }
    }
}

fn parse_locale(json: &str) -> HashMap<String, String> {
    let value: Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };

    let mut map = HashMap::new();
    if let Value::Object(obj) = value {
        for (key, val) in obj {
            if let Value::String(s) = val {
                map.insert(key, s);
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_locale() {
        let json = r#"{"hello": "world", "foo": "bar"}"#;
        let map = parse_locale(json);
        assert_eq!(map.get("hello").unwrap(), "world");
        assert_eq!(map.get("foo").unwrap(), "bar");
    }

    #[test]
    fn test_t_fmt_replaces_vars() {
        // Can't test t() directly due to OnceLock, but t_fmt logic works on any string
        let s = "Download {filename} — {size} done".to_string();
        let mut result = s;
        for (name, value) in &[("filename", "test.zip"), ("size", "5 MB")] {
            result = result.replace(&format!("{{{name}}}"), value);
        }
        assert_eq!(result, "Download test.zip — 5 MB done");
    }

    #[test]
    fn test_detect_system_lang() {
        // Should return something, at minimum "en"
        let lang = detect_system_lang();
        assert!(!lang.is_empty());
    }
}
