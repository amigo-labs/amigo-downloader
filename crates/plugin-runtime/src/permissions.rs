//! Per-plugin capability scoping.
//!
//! A plugin may declare `permissions.domains` — the hosts it is allowed to
//! reach through `amigo.http*`. The list is validated once at load time,
//! compiled into a [`DomainAllowlist`] and bound into that plugin's host-API
//! bindings, where JS cannot see or change it. Every request (and every
//! redirect hop) is checked against it in `HostApi::check_url_allowed`.

/// Maximum number of entries a plugin may declare.
const MAX_DOMAIN_ENTRIES: usize = 64;

/// A single compiled host pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
enum HostPattern {
    /// `api.real-debrid.com` — matches that host only.
    Exact(String),
    /// `*.rdeb.io` — matches any subdomain of `rdeb.io`, not `rdeb.io` itself.
    Subdomains(String),
}

/// The compiled, validated set of hosts a plugin may reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainAllowlist {
    patterns: Vec<HostPattern>,
}

impl DomainAllowlist {
    /// Validate and compile declared host patterns.
    ///
    /// Accepted: a hostname (`api.example.com`) or a single leading wildcard
    /// label (`*.example.com`). Rejected: schemes, paths, ports, user info,
    /// IP-literal brackets, a bare `*`, wildcards anywhere but the first label,
    /// and wildcards over a single label (`*.com`).
    pub fn parse(entries: &[String]) -> Result<Self, String> {
        if entries.len() > MAX_DOMAIN_ENTRIES {
            return Err(format!(
                "permissions.domains has {} entries (limit {MAX_DOMAIN_ENTRIES})",
                entries.len()
            ));
        }
        let patterns = entries
            .iter()
            .map(|e| parse_pattern(e))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { patterns })
    }

    /// Whether `host` (as parsed out of a request URL) is covered.
    pub fn allows(&self, host: &str) -> bool {
        let host = normalize_host(host);
        self.patterns.iter().any(|p| match p {
            HostPattern::Exact(h) => *h == host,
            // `base` is stored with its leading dot (".rdeb.io"), so a
            // non-empty prefix means a proper subdomain.
            HostPattern::Subdomains(base) => host
                .strip_suffix(base.as_str())
                .is_some_and(|prefix| !prefix.is_empty()),
        })
    }
}

/// Lowercase and drop a trailing root dot, so `API.Example.com.` and
/// `api.example.com` compare equal.
fn normalize_host(host: &str) -> String {
    host.trim_end_matches('.').to_ascii_lowercase()
}

fn parse_pattern(raw: &str) -> Result<HostPattern, String> {
    let entry = raw.trim();
    let reject = |why: &str| Err(format!("invalid permissions.domains entry {raw:?}: {why}"));

    if entry.is_empty() {
        return reject("empty");
    }
    if entry == "*" {
        return reject("a bare \"*\" is not allowed — list the hosts the plugin needs");
    }
    if entry.contains("://") || entry.contains('/') {
        return reject("must be a host, without scheme or path");
    }
    if entry.contains(':') || entry.contains('@') || entry.contains('[') {
        return reject("must be a host name, without port, user info or IP brackets");
    }

    let (wildcard, host) = match entry.strip_prefix("*.") {
        Some(rest) => (true, rest),
        None => (false, entry),
    };
    if host.contains('*') {
        return reject("\"*\" is only allowed as the whole first label (\"*.example.com\")");
    }

    let host = normalize_host(host);
    let labels: Vec<&str> = host.split('.').collect();
    let label_ok = |l: &&str| {
        !l.is_empty()
            && l.len() <= 63
            && !l.starts_with('-')
            && !l.ends_with('-')
            && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    };
    if host.len() > 253 || !labels.iter().all(label_ok) {
        return reject("not a valid host name");
    }
    if wildcard && labels.len() < 2 {
        return reject("a wildcard must cover at least a registrable domain (\"*.example.com\")");
    }

    Ok(if wildcard {
        HostPattern::Subdomains(format!(".{host}"))
    } else {
        HostPattern::Exact(host)
    })
}

/// Compare a previously approved domain set with a new one and return the
/// entries that are new. An unscoped (`None`) new set counts as widening any
/// scoped old set; any new set is a narrowing of an unscoped old one.
pub fn widened_domains(old: Option<&[String]>, new: Option<&[String]>) -> Option<Vec<String>> {
    match (old, new) {
        (None, _) => None,
        (Some(_), None) => Some(vec!["*".to_string()]),
        (Some(old), Some(new)) => {
            let old: Vec<String> = old.iter().map(|d| normalize_host(d.trim())).collect();
            let added: Vec<String> = new
                .iter()
                .filter(|d| !old.contains(&normalize_host(d.trim())))
                .cloned()
                .collect();
            if added.is_empty() { None } else { Some(added) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(entries: &[&str]) -> DomainAllowlist {
        DomainAllowlist::parse(&entries.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn exact_host_matches_only_itself() {
        let l = list(&["api.real-debrid.com"]);
        assert!(l.allows("api.real-debrid.com"));
        assert!(l.allows("API.Real-Debrid.com."));
        assert!(!l.allows("real-debrid.com"));
        assert!(!l.allows("evil.api.real-debrid.com"));
        assert!(!l.allows("api.real-debrid.com.evil.net"));
    }

    #[test]
    fn wildcard_matches_subdomains_only() {
        let l = list(&["*.rdeb.io"]);
        assert!(l.allows("a.rdeb.io"));
        assert!(l.allows("x.y.rdeb.io"));
        assert!(!l.allows("rdeb.io"));
        assert!(!l.allows("evilrdeb.io"));
    }

    #[test]
    fn empty_list_allows_nothing() {
        let l = list(&[]);
        assert!(!l.allows("example.com"));
    }

    #[test]
    fn rejects_malformed_entries() {
        for bad in [
            "*",
            "",
            "https://example.com",
            "example.com/path",
            "example.com:443",
            "user@example.com",
            "[::1]",
            "a.*.example.com",
            "*example.com",
            "*.com",
            "exa mple.com",
            "-bad.com",
        ] {
            assert!(
                DomainAllowlist::parse(&[bad.to_string()]).is_err(),
                "{bad:?} must be rejected"
            );
        }
    }

    #[test]
    fn widened_domains_detects_growth() {
        let a = vec!["a.com".to_string()];
        let ab = vec!["a.com".to_string(), "b.com".to_string()];
        assert_eq!(
            widened_domains(Some(&a), Some(&ab)),
            Some(vec!["b.com".into()])
        );
        assert_eq!(widened_domains(Some(&ab), Some(&a)), None);
        assert_eq!(widened_domains(Some(&a), Some(&a)), None);
        assert_eq!(widened_domains(Some(&a), None), Some(vec!["*".into()]));
        assert_eq!(widened_domains(None, Some(&a)), None);
        assert_eq!(widened_domains(None, None), None);
    }
}
