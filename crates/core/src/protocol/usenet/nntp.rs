//! NNTP/NNTPS client for Usenet article retrieval.
//!
//! Implements the NNTP protocol (RFC 3977) over TCP/TLS.
//! Supports authentication, GROUP, ARTICLE, and BODY commands.

use std::io;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::{debug, warn};

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

        let tcp = TcpStream::connect((&*config.host, config.port))
            .await
            .map_err(|e| crate::Error::Other(format!("NNTP connect failed: {e}")))?;

        let (reader, writer): (
            Box<dyn tokio::io::AsyncRead + Unpin + Send>,
            Box<dyn tokio::io::AsyncWrite + Unpin + Send>,
        ) = if config.ssl {
            let connector = tokio_native_tls::TlsConnector::from(
                native_tls::TlsConnector::new()
                    .map_err(|e| crate::Error::Other(format!("TLS error: {e}")))?,
            );
            let tls = connector
                .connect(&config.host, tcp)
                .await
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
        self.writer
            .write_all(format!("{cmd}\r\n").as_bytes())
            .await
            .map_err(|e| crate::Error::Other(format!("NNTP write error: {e}")))?;
        self.writer
            .flush()
            .await
            .map_err(|e| crate::Error::Other(format!("NNTP flush error: {e}")))?;
        Ok(())
    }

    /// Read a single-line NNTP response.
    async fn read_response(&mut self) -> Result<NntpResponse, crate::Error> {
        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .await
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
            let n = self
                .reader
                .read_line(&mut line)
                .await
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
