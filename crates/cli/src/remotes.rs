//! Persist remote amigo servers + their API tokens in
//! `$AMIGO_CONFIG_DIR/remotes.toml` (falling back to
//! `~/.config/amigo/remotes.toml`).
//!
//! Tokens are stored in plaintext for now — the file should be `0600`. That
//! matches how `gh`, `helm`, and friends handle credentials at this tier.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Remote {
    pub url: String,
    pub token: String,
    #[serde(default)]
    pub device_name: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Remotes {
    /// Alias ("laptop", "nas", …) → remote.
    #[serde(default)]
    pub remotes: BTreeMap<String, Remote>,
    /// Alias of the default remote; honoured when `--remote` isn't passed.
    #[serde(default)]
    pub default: Option<String>,
}

pub fn config_path() -> PathBuf {
    if let Ok(dir) = std::env::var("AMIGO_CONFIG_DIR") {
        return PathBuf::from(dir).join("remotes.toml");
    }
    if let Some(home) = dirs_home() {
        return home.join(".config").join("amigo").join("remotes.toml");
    }
    PathBuf::from("remotes.toml")
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

pub fn load() -> Remotes {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => toml::from_str(&s).unwrap_or_default(),
        Err(_) => Remotes::default(),
    }
}

pub fn save(remotes: &Remotes) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let s = toml::to_string_pretty(remotes).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(&path, s).map_err(|e| format!("write {}: {e}", path.display()))?;
    // Best-effort chmod 0600 on unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o600);
            let _ = std::fs::set_permissions(&path, perms);
        }
    }
    Ok(())
}

/// Pick a remote by alias, falling back to `remotes.default`. The returned
/// alias matches the key in `remotes.remotes`. Used by `--remote`-aware
/// subcommands (threading is in-progress — see audit plan).
#[allow(dead_code)]
pub fn resolve(remotes: &Remotes, alias: Option<&str>) -> Option<(String, Remote)> {
    let key = alias
        .map(|s| s.to_string())
        .or_else(|| remotes.default.clone())?;
    remotes.remotes.get(&key).cloned().map(|r| (key, r))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_toml() {
        let mut r = Remotes::default();
        r.remotes.insert(
            "nas".into(),
            Remote {
                url: "http://nas:1516".into(),
                token: "abc".into(),
                device_name: Some("laptop".into()),
            },
        );
        r.default = Some("nas".into());
        let s = toml::to_string_pretty(&r).unwrap();
        let back: Remotes = toml::from_str(&s).unwrap();
        assert_eq!(back.default.as_deref(), Some("nas"));
        assert_eq!(back.remotes.get("nas").unwrap().token, "abc");
    }

    #[test]
    fn resolve_default_falls_back_when_alias_missing() {
        let mut r = Remotes::default();
        r.remotes.insert(
            "nas".into(),
            Remote {
                url: "http://nas:1516".into(),
                token: "tok".into(),
                device_name: None,
            },
        );
        r.default = Some("nas".into());
        let (name, _) = resolve(&r, None).unwrap();
        assert_eq!(name, "nas");
    }
}
