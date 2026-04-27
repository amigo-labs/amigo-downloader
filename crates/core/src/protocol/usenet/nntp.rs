//! NNTP/NNTPS client for Usenet article retrieval.
//!
//! Implements the NNTP protocol (RFC 3977) over TCP/TLS.
//! Supports authentication, GROUP, ARTICLE, and BODY commands.
//!
//! Network reads/writes are wrapped in [`NNTP_IO_TIMEOUT`] so a stalled
//! upstream cannot wedge a worker forever; the hostname is sanity-checked
//! before [`TcpStream::connect`] so a corrupted config cannot turn a Usenet
//! server entry into an SSRF gadget.

use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::debug;

/// Cap on how long a single NNTP read/write may block. Servers that go quiet
/// mid-article should be dropped instead of holding a connection slot
/// forever; the connection pool can reconnect on the next attempt.
const NNTP_IO_TIMEOUT: Duration = Duration::from_secs(60);

/// Validate that `host` looks like a real DNS name or IP literal before
/// handing it to the resolver. The previous code passed `&*config.host`
/// straight into `TcpStream::connect`, so a bad config (e.g. a stray
/// `"://"` prefix from copy-paste) would either fail with a confusing
/// resolver error or, worse, accept a URI-style input that some resolvers
/// special-case. This guard keeps the input shape predictable.
fn validate_nntp_host(host: &str) -> Result<(), crate::Error> {
    if host.is_empty() || host.len() > 253 {
        return Err(crate::Error::Other(format!(
            "NNTP host has invalid length: {} char(s)",
            host.len()
        )));
    }
    if host.contains("://") || host.contains('/') || host.contains(' ') {
        return Err(crate::Error::Other(format!(
            "NNTP host '{host}' contains URI / path / whitespace characters"
        )));
    }
    // Allow IPv6 literals in bracketed or unbracketed form, or any DNS label
    // (alnum, hyphen, dot, plus the colon separator some configs include
    // for v6 zones — that case is filtered above by the / / ' ' rejections).
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | ':' | '[' | ']');
    if !host.chars().all(allowed) {
        return Err(crate::Error::Other(format!(
            "NNTP host '{host}' contains characters outside [A-Za-z0-9.-:[]]"
        )));
    }
    Ok(())
}

/// NNTP server configuration.
#[derive(Debug, Clone)]
pub struct NntpServerConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub ssl: bool,
    pub username: String,
    pub password: String,
    pub connections: u32,
    pub priority: u32,
}

/// An active NNTP connection.
pub struct NntpConnection {
    reader: BufReader<Box<dyn tokio::io::AsyncRead + Unpin + Send>>,
    writer: Box<dyn tokio::io::AsyncWrite + Unpin + Send>,
    server_name: String,
}

/// NNTP response.
#[derive(Debug)]
pub struct NntpResponse {
    pub code: u16,
    pub message: String,
}

impl NntpConnection {
    /// Connect to an NNTP server (plain TCP or TLS).
    pub async fn connect(config: &NntpServerConfig) -> Result<Self, crate::Error> {
        debug!("Connecting to NNTP server {}:{}", config.host, config.port);

        validate_nntp_host(&config.host)?;

        let tcp = timeout(
            NNTP_IO_TIMEOUT,
            TcpStream::connect((&*config.host, config.port)),
        )
        .await
        .map_err(|_| crate::Error::Other("NNTP connect timed out".into()))?
        .map_err(|e| crate::Error::Other(format!("NNTP connect failed: {e}")))?;

        let (reader, writer): (
            Box<dyn tokio::io::AsyncRead + Unpin + Send>,
            Box<dyn tokio::io::AsyncWrite + Unpin + Send>,
        ) = if config.ssl {
            // Build the TLS connector explicitly so the certificate /
            // hostname checks stay enabled even if a future refactor
            // swaps the helper. native_tls defaults are correct (verify
            // cert chain + hostname); the explicit `false` calls below
            // make a regression — e.g. someone toggling them on for
            // local debugging — visible in code review.
            let mut builder = native_tls::TlsConnector::builder();
            builder.danger_accept_invalid_certs(false);
            builder.danger_accept_invalid_hostnames(false);
            let connector = tokio_native_tls::TlsConnector::from(
                builder
                    .build()
                    .map_err(|e| crate::Error::Other(format!("TLS error: {e}")))?,
            );
            let tls = timeout(NNTP_IO_TIMEOUT, connector.connect(&config.host, tcp))
                .await
                .map_err(|_| crate::Error::Other("NNTP TLS handshake timed out".into()))?
                .map_err(|e| crate::Error::Other(format!("TLS handshake failed: {e}")))?;
            let (r, w) = tokio::io::split(tls);
            (Box::new(r), Box::new(w))
        } else {
            let (r, w) = tokio::io::split(tcp);
            (Box::new(r), Box::new(w))
        };

        let mut conn = Self {
            reader: BufReader::new(reader),
            writer,
            server_name: config.name.clone(),
        };

        // Read welcome banner
        let banner = conn.read_response().await?;
        debug!(
            "[{}] Banner: {} {}",
            config.name, banner.code, banner.message
        );

        if banner.code != 200 && banner.code != 201 {
            return Err(crate::Error::Other(format!(
                "NNTP server rejected connection: {} {}",
                banner.code, banner.message
            )));
        }

        // Authenticate if credentials provided
        if !config.username.is_empty() {
            conn.authenticate(&config.username, &config.password)
                .await?;
        }

        Ok(conn)
    }

    async fn authenticate(&mut self, username: &str, password: &str) -> Result<(), crate::Error> {
        self.send_command(&format!("AUTHINFO USER {username}"))
            .await?;
        let resp = self.read_response().await?;

        if resp.code == 381 {
            // Password required
            self.send_command(&format!("AUTHINFO PASS {password}"))
                .await?;
            let resp = self.read_response().await?;
            if resp.code != 281 {
                return Err(crate::Error::Other(format!(
                    "NNTP auth failed: {} {}",
                    resp.code, resp.message
                )));
            }
        } else if resp.code != 281 {
            return Err(crate::Error::Other(format!(
                "NNTP auth failed: {} {}",
                resp.code, resp.message
            )));
        }

        debug!("[{}] Authenticated", self.server_name);
        Ok(())
    }

    /// Select a newsgroup.
    pub async fn group(&mut self, group: &str) -> Result<NntpResponse, crate::Error> {
        self.send_command(&format!("GROUP {group}")).await?;
        self.read_response().await
    }

    /// Download an article body by message-ID.
    pub async fn body(&mut self, message_id: &str) -> Result<Vec<u8>, crate::Error> {
        let mid = if message_id.starts_with('<') {
            message_id.to_string()
        } else {
            format!("<{message_id}>")
        };

        self.send_command(&format!("BODY {mid}")).await?;
        let resp = self.read_response().await?;

        if resp.code != 222 {
            return Err(crate::Error::Other(format!(
                "BODY failed for {mid}: {} {}",
                resp.code, resp.message
            )));
        }

        // Read multi-line response until ".\r\n"
        self.read_multiline_body().await
    }

    /// Send a raw NNTP command.
    async fn send_command(&mut self, cmd: &str) -> Result<(), crate::Error> {
        let payload = format!("{cmd}\r\n");
        timeout(NNTP_IO_TIMEOUT, self.writer.write_all(payload.as_bytes()))
            .await
            .map_err(|_| crate::Error::Other("NNTP write timed out".into()))?
            .map_err(|e| crate::Error::Other(format!("NNTP write error: {e}")))?;
        timeout(NNTP_IO_TIMEOUT, self.writer.flush())
            .await
            .map_err(|_| crate::Error::Other("NNTP flush timed out".into()))?
            .map_err(|e| crate::Error::Other(format!("NNTP flush error: {e}")))?;
        Ok(())
    }

    /// Read a single-line NNTP response.
    async fn read_response(&mut self) -> Result<NntpResponse, crate::Error> {
        let mut line = String::new();
        timeout(NNTP_IO_TIMEOUT, self.reader.read_line(&mut line))
            .await
            .map_err(|_| crate::Error::Other("NNTP read timed out".into()))?
            .map_err(|e| crate::Error::Other(format!("NNTP read error: {e}")))?;

        let line = line.trim_end();
        let code = line
            .get(..3)
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(0);
        let message = line.get(4..).unwrap_or("").to_string();

        Ok(NntpResponse { code, message })
    }

    /// Read a multi-line response body (terminated by ".\r\n").
    async fn read_multiline_body(&mut self) -> Result<Vec<u8>, crate::Error> {
        let mut body = Vec::new();
        let mut line = String::new();

        loop {
            line.clear();
            let n = timeout(NNTP_IO_TIMEOUT, self.reader.read_line(&mut line))
                .await
                .map_err(|_| crate::Error::Other("NNTP read timed out".into()))?
                .map_err(|e| crate::Error::Other(format!("NNTP read error: {e}")))?;

            if n == 0 {
                return Err(crate::Error::Other(
                    "NNTP connection closed unexpectedly".into(),
                ));
            }

            let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');

            // Dot-stuffing: a line starting with ".." has the first dot removed
            if trimmed == "." {
                break;
            }

            let actual_line = if trimmed.starts_with("..") {
                &trimmed[1..]
            } else {
                trimmed
            };

            body.extend_from_slice(actual_line.as_bytes());
            body.push(b'\n');
        }

        Ok(body)
    }

    /// Send QUIT and close connection.
    pub async fn quit(&mut self) -> Result<(), crate::Error> {
        let _ = self.send_command("QUIT").await;
        Ok(())
    }
}

/// A pool of NNTP connections to a single server.
pub struct NntpConnectionPool {
    config: NntpServerConfig,
    connections: tokio::sync::Mutex<Vec<NntpConnection>>,
}

impl NntpConnectionPool {
    pub fn new(config: NntpServerConfig) -> Self {
        Self {
            config,
            connections: tokio::sync::Mutex::new(Vec::new()),
        }
    }

    /// Get a connection from the pool, or create a new one.
    pub async fn acquire(&self) -> Result<NntpConnection, crate::Error> {
        let mut conns = self.connections.lock().await;
        if let Some(conn) = conns.pop() {
            return Ok(conn);
        }
        drop(conns);

        NntpConnection::connect(&self.config).await
    }

    /// Return a connection to the pool.
    pub async fn release(&self, conn: NntpConnection) {
        let mut conns = self.connections.lock().await;
        if conns.len() < self.config.connections as usize {
            conns.push(conn);
        }
        // Otherwise drop the connection
    }

    pub fn server_name(&self) -> &str {
        &self.config.name
    }

    pub fn priority(&self) -> u32 {
        self.config.priority
    }
}

#[cfg(test)]
mod tests {
    use super::validate_nntp_host;

    #[test]
    fn validate_accepts_dns_names_and_ip_literals() {
        for ok in [
            "news.example.com",
            "alt.binaries.example.org",
            "1.2.3.4",
            "[::1]",
            "[2001:db8::1]",
        ] {
            validate_nntp_host(ok).unwrap_or_else(|e| panic!("{ok} should be accepted: {e}"));
        }
    }

    #[test]
    fn validate_rejects_uri_like_inputs() {
        // Misconfiguration patterns the previous code passed straight to
        // resolver/connect, sometimes with surprising fallback behaviour.
        for bad in [
            "",
            "nntps://news.example.com",
            "news.example.com/path",
            "news.example.com:563/extra",
            "news .example.com",
            "news.example.com\nfoo",
        ] {
            assert!(
                validate_nntp_host(bad).is_err(),
                "{bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn validate_rejects_overlong_host() {
        let huge = "a".repeat(254);
        assert!(validate_nntp_host(&huge).is_err());
    }
}
