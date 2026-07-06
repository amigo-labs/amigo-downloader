//! Outbound-URL guard for server-initiated HTTP requests.
//!
//! Server features (webhook dispatch, RSS feed polling, plugin marketplace,
//! self-update, feedback) issue HTTP requests to operator-supplied URLs.
//! Without a guard those URLs can target loopback / RFC1918 / link-local /
//! cloud-metadata addresses, turning the server into an SSRF gadget that
//! probes internal services or exfiltrates data.
//!
//! [`validate_outbound_url`] resolves the URL's host (including IP literals)
//! and rejects schemes other than `http`/`https`, malformed URLs, and any
//! resolved address inside a non-public block. Call it at request time, not
//! once at config save: DNS-rebinding attacks rely on changing the answer
//! between validation and the actual fetch.
//!
//! Mirrors the plugin-runtime `host_api::is_blocked_ip` policy so plugins,
//! webhooks, and RSS feeds all see the same allowlist.

use std::net::IpAddr;

use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum GuardError {
    #[error("URL is malformed: {0}")]
    Malformed(String),
    #[error("URL scheme '{0}' is not allowed (must be http or https)")]
    BadScheme(String),
    #[error("URL has no host component")]
    MissingHost,
    #[error("DNS resolution failed for {host}: {source}")]
    DnsFailed {
        host: String,
        #[source]
        source: std::io::Error,
    },
    #[error("URL resolves to a non-public address ({addr}) — refusing to issue request")]
    BlockedAddress { addr: IpAddr },
}

/// Validate that `url` is safe to fetch from the server.
///
/// `allow_private_targets = true` skips the IP-block list, intended for
/// operators who explicitly need to reach LAN hosts (e.g. a self-hosted
/// webhook receiver). Scheme validation still runs.
pub async fn validate_outbound_url(
    url: &str,
    allow_private_targets: bool,
) -> Result<(), GuardError> {
    let parsed = Url::parse(url).map_err(|e| GuardError::Malformed(e.to_string()))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => return Err(GuardError::BadScheme(other.to_string())),
    }
    let host = parsed.host().ok_or(GuardError::MissingHost)?;
    if allow_private_targets {
        return Ok(());
    }
    let port = parsed.port_or_known_default().unwrap_or(80);

    // url::Host distinguishes IP literals from DNS names so we can check IP
    // forms directly without having to strip the IPv6 brackets that
    // host_str() returns.
    let dns_target = match host {
        url::Host::Ipv4(ip) => {
            let ip = IpAddr::V4(ip);
            if is_blocked_ip(ip) {
                return Err(GuardError::BlockedAddress { addr: ip });
            }
            return Ok(());
        }
        url::Host::Ipv6(ip) => {
            let ip = IpAddr::V6(ip);
            if is_blocked_ip(ip) {
                return Err(GuardError::BlockedAddress { addr: ip });
            }
            return Ok(());
        }
        url::Host::Domain(d) => d.to_string(),
    };

    let addrs = tokio::net::lookup_host((dns_target.as_str(), port))
        .await
        .map_err(|e| GuardError::DnsFailed {
            host: dns_target.clone(),
            source: e,
        })?;
    for addr in addrs {
        if is_blocked_ip(addr.ip()) {
            return Err(GuardError::BlockedAddress { addr: addr.ip() });
        }
    }
    Ok(())
}

/// Return `true` for any address that is not safely reachable as a public
/// destination: loopback, RFC1918, CGNAT, link-local, broadcast, the AWS/GCP
/// metadata IP (169.254.169.254), unique-local IPv6, and IPv6-mapped versions
/// of any of the above.
fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            if v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.is_documentation()
                || v4.is_multicast()
            {
                return true;
            }
            let o = v4.octets();
            // "This host on this network" 0.0.0.0/8 — 0.x.y.z routes to
            // localhost on Linux, so it is not a safe arbitrary target.
            if o[0] == 0 {
                return true;
            }
            // Reserved for future use 240.0.0.0/4 (255.255.255.255 is already
            // caught by is_broadcast).
            if o[0] >= 240 {
                return true;
            }
            // Benchmarking 198.18.0.0/15.
            if o[0] == 198 && (18..=19).contains(&o[1]) {
                return true;
            }
            // CGNAT 100.64.0.0/10 — used by ISPs for shared IPv4, not safe
            // as an arbitrary target.
            o[0] == 100 && (64..=127).contains(&o[1])
        }
        IpAddr::V6(v6) => {
            if v6.is_loopback() || v6.is_unspecified() || v6.is_multicast() {
                return true;
            }
            let segs = v6.segments();
            // Unique-local fc00::/7
            if (segs[0] & 0xfe00) == 0xfc00 {
                return true;
            }
            // Link-local fe80::/10
            if (segs[0] & 0xffc0) == 0xfe80 {
                return true;
            }
            // IPv4-mapped (::ffff:a.b.c.d) and IPv4-compatible (::a.b.c.d):
            // inherit the embedded address's verdict so `::127.0.0.1` and
            // `::ffff:169.254.169.254` cannot slip through.
            if let Some(v4) = v6.to_ipv4() {
                return is_blocked_ip(IpAddr::V4(v4));
            }
            // The transition prefixes below all embed an IPv4 address; block
            // them when that embedded address is itself non-public.
            if let Some(v4) = embedded_v4(segs) {
                return is_blocked_ip(IpAddr::V4(v4));
            }
            false
        }
    }
}

/// Extract the IPv4 address embedded in an IPv6 transition prefix (NAT64
/// `64:ff9b::/96`, 6to4 `2002::/16`, Teredo `2001:0000::/32`), if any. These
/// prefixes tunnel v4 traffic and would otherwise bypass the v4 block list.
fn embedded_v4(segs: [u16; 8]) -> Option<std::net::Ipv4Addr> {
    let v4 = |hi: u16, lo: u16| {
        std::net::Ipv4Addr::new(
            (hi >> 8) as u8,
            (hi & 0xff) as u8,
            (lo >> 8) as u8,
            (lo & 0xff) as u8,
        )
    };
    // NAT64 well-known prefix 64:ff9b::/96 — v4 in the low 32 bits.
    if segs[0] == 0x0064 && segs[1] == 0xff9b {
        return Some(v4(segs[6], segs[7]));
    }
    // 6to4 2002::/16 — v4 in segments 1..=2.
    if segs[0] == 0x2002 {
        return Some(v4(segs[1], segs[2]));
    }
    // Teredo 2001:0000::/32 — obfuscated (bitwise-NOT) client v4 in the last
    // two segments.
    if segs[0] == 0x2001 && segs[1] == 0x0000 {
        return Some(v4(segs[6] ^ 0xffff, segs[7] ^ 0xffff));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_loopback_literal() {
        let err = validate_outbound_url("http://127.0.0.1/foo", false)
            .await
            .expect_err("loopback must be blocked");
        assert!(matches!(err, GuardError::BlockedAddress { .. }), "{err}");
    }

    #[tokio::test]
    async fn rejects_aws_metadata_ip() {
        let err = validate_outbound_url("http://169.254.169.254/latest/meta-data/", false)
            .await
            .expect_err("AWS metadata IP must be blocked");
        assert!(matches!(err, GuardError::BlockedAddress { .. }), "{err}");
    }

    #[tokio::test]
    async fn rejects_rfc1918_literal() {
        for host in ["10.0.0.1", "192.168.0.1", "172.16.0.1"] {
            let url = format!("https://{host}/x");
            let err = validate_outbound_url(&url, false)
                .await
                .expect_err("RFC1918 must be blocked");
            assert!(matches!(err, GuardError::BlockedAddress { .. }), "{err}");
        }
    }

    #[tokio::test]
    async fn rejects_ipv6_loopback_and_ula() {
        for url in ["http://[::1]/x", "http://[fc00::1]/x", "http://[fe80::1]/x"] {
            let err = validate_outbound_url(url, false)
                .await
                .expect_err("non-public v6 must be blocked");
            assert!(
                matches!(err, GuardError::BlockedAddress { .. }),
                "{url} → {err}"
            );
        }
    }

    #[tokio::test]
    async fn rejects_unsupported_scheme() {
        for url in [
            "file:///etc/passwd",
            "gopher://attacker.example/x",
            "ftp://intranet/secret",
            "data:text/plain,hi",
        ] {
            let err = validate_outbound_url(url, false)
                .await
                .expect_err("scheme must be blocked");
            assert!(matches!(err, GuardError::BadScheme(_)), "{url} → {err}");
        }
    }

    #[tokio::test]
    async fn allow_private_skips_ip_check_but_still_checks_scheme() {
        // Loopback is allowed when caller opts into private targets.
        validate_outbound_url("http://127.0.0.1/x", true)
            .await
            .expect("loopback should be allowed when opted in");
        // But unsupported schemes are still rejected.
        let err = validate_outbound_url("file:///etc/passwd", true)
            .await
            .expect_err("scheme check still applies");
        assert!(matches!(err, GuardError::BadScheme(_)), "{err}");
    }

    #[tokio::test]
    async fn rejects_malformed_url() {
        let err = validate_outbound_url("not a url at all", false)
            .await
            .expect_err("must reject malformed");
        assert!(matches!(err, GuardError::Malformed(_)), "{err}");
    }

    #[test]
    fn ipv4_mapped_v6_inherits_v4_block() {
        let ip: IpAddr = "::ffff:127.0.0.1".parse().unwrap();
        assert!(is_blocked_ip(ip));
        let ip: IpAddr = "::ffff:169.254.169.254".parse().unwrap();
        assert!(is_blocked_ip(ip));
    }

    #[test]
    fn blocks_extra_ipv4_ranges() {
        for s in [
            "0.0.0.1",         // 0.0.0.0/8
            "0.1.2.3",         // 0.0.0.0/8
            "224.0.0.1",       // multicast
            "239.255.255.250", // multicast (SSDP)
            "240.0.0.1",       // reserved 240/4
            "198.18.0.1",      // benchmarking
            "198.19.255.255",  // benchmarking
        ] {
            let ip: IpAddr = s.parse().unwrap();
            assert!(is_blocked_ip(ip), "{s} should be blocked");
        }
        // A normal public address is still allowed.
        assert!(!is_blocked_ip("1.1.1.1".parse().unwrap()));
        assert!(!is_blocked_ip("198.20.0.1".parse().unwrap()));
    }

    #[test]
    fn blocks_ipv6_transition_prefixes_embedding_private_v4() {
        // IPv4-compatible ::a.b.c.d
        assert!(is_blocked_ip("::127.0.0.1".parse().unwrap()));
        // NAT64 64:ff9b::/96 embedding loopback
        assert!(is_blocked_ip("64:ff9b::7f00:1".parse().unwrap()));
        // 6to4 2002::/16 embedding RFC1918 10.0.0.1 → 2002:0a00:0001::
        assert!(is_blocked_ip("2002:a00:1::".parse().unwrap()));
        // IPv6 multicast ff00::/8
        assert!(is_blocked_ip("ff02::1".parse().unwrap()));
    }
}
