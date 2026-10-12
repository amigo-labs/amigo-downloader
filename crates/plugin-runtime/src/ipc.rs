//! Wire protocol between the daemon and the `amigo-plugin-host` child.
//!
//! Framing: a `u32` big-endian length followed by that many bytes of JSON.
//! The child writes [`ToParent::Hello`] first; every other frame is a
//! [`ToChild`] (on the child's stdin) or a [`ToParent`] (on its stdout).
//!
//! Requests carry an `id` chosen by the parent and answered by exactly one
//! [`ToParent::Response`]. While a request runs, the child may issue any
//! number of [`ToParent::HostCall`]s — each answered by one
//! [`ToChild::HostReply`] — to execute the stateful host functions that live
//! in the daemon. Host calls have no timeout: `solveCaptcha` waits for a
//! human. The plugin's own deadline is enforced in the child.

use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};

use crate::host_fns::{HostArgs, HostResult};
use crate::sandbox::SandboxLimits;
use crate::types::PluginMeta;

/// Bumped on any incompatible change to the messages below. The daemon
/// refuses a child that reports a different version.
pub const PROTOCOL_VERSION: u32 = 1;

/// Largest accepted frame. Covers a 32 MiB HTTP body (the host-side cap)
/// after JSON escaping, or a base64 `httpGetBinary` payload of the same size.
pub const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;

/// Opaque id of one loaded plugin context inside the child.
pub type Handle = u64;

/// Parent → child.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ToChild {
    Request {
        id: u64,
        body: Request,
    },
    HostReply {
        call_id: u64,
        result: HostResult,
    },
    /// Free a context. Fire-and-forget; unknown handles are ignored.
    Drop {
        handle: Handle,
    },
}

/// Work the parent asks the child to do.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Request {
    /// Evaluate a plugin's source (transpiling TypeScript) in a fresh
    /// context, read its manifest and bind the host API. `cached_bytecode`
    /// (base64) is tried first; `want_bytecode` asks for the compiled module
    /// back so the parent can cache it.
    Load {
        source: String,
        filename: String,
        is_typescript: bool,
        cached_bytecode: Option<String>,
        want_bytecode: bool,
        limits: SandboxLimits,
    },
    Resolve {
        handle: Handle,
        url: String,
        timeout_ms: u64,
    },
    PostProcess {
        handle: Handle,
        context_json: String,
        timeout_ms: u64,
    },
    RunSpec {
        handle: Handle,
        spec_source: String,
        filename: String,
        is_typescript: bool,
        timeout_ms: u64,
    },
}

/// Child → parent.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ToParent {
    Hello {
        protocol: u32,
    },
    Response {
        id: u64,
        result: Result<Payload, crate::Error>,
    },
    /// Execute host function `name` for the plugin loaded as `handle`, on
    /// behalf of request `request`.
    HostCall {
        call_id: u64,
        request: u64,
        handle: Handle,
        name: String,
        args: HostArgs,
    },
}

/// Successful result of a [`Request`].
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Payload {
    Loaded {
        handle: Handle,
        meta: Box<PluginMeta>,
        /// base64 bytecode compiled from source, when `want_bytecode` was set
        /// and the module was not loaded from `cached_bytecode`.
        new_bytecode: Option<String>,
        /// `cached_bytecode` was sent but could not be evaluated; the parent
        /// drops the cache entry.
        cached_bytecode_rejected: bool,
        /// Non-fatal manifest findings for the daemon to log.
        warnings: Vec<String>,
    },
    /// JSON text returned by `resolve` / `postProcess`. `None` from
    /// `PostProcess` means the plugin has no `postProcess` hook.
    Json {
        json: Option<String>,
    },
    Spec {
        results: crate::engine::TestResults,
    },
}

/// Write one frame.
pub fn write_frame<W: Write, T: Serialize>(w: &mut W, msg: &T) -> io::Result<()> {
    let body = serde_json::to_vec(msg).map_err(io::Error::other)?;
    if body.len() > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("frame of {} bytes exceeds {MAX_FRAME_BYTES}", body.len()),
        ));
    }
    let len = u32::try_from(body.len()).map_err(io::Error::other)?;
    w.write_all(&len.to_be_bytes())?;
    w.write_all(&body)?;
    w.flush()
}

/// Read one frame. `Ok(None)` on a clean EOF before the length prefix.
pub fn read_frame<R: Read, T: for<'de> Deserialize<'de>>(r: &mut R) -> io::Result<Option<T>> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let body = read_body(r, u32::from_be_bytes(len))?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn read_body<R: Read>(r: &mut R, len: u32) -> io::Result<Vec<u8>> {
    let len = len as usize;
    if len > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame of {len} bytes exceeds {MAX_FRAME_BYTES}"),
        ));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    Ok(body)
}

/// Async counterpart of [`read_frame`] for the daemon side.
pub async fn read_frame_async<R, T>(r: &mut R) -> io::Result<Option<T>>
where
    R: tokio::io::AsyncRead + Unpin,
    T: for<'de> Deserialize<'de>,
{
    use tokio::io::AsyncReadExt;

    let mut len = [0u8; 4];
    match r.read_exact(&mut len).await {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame of {len} bytes exceeds {MAX_FRAME_BYTES}"),
        ));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).await?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Encode one frame for an async writer.
pub fn encode_frame<T: Serialize>(msg: &T) -> io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    write_frame(&mut buf, msg)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip() {
        let mut buf = Vec::new();
        let msg = ToChild::HostReply {
            call_id: 7,
            result: Ok(Some("ok".into())),
        };
        write_frame(&mut buf, &msg).unwrap();
        write_frame(&mut buf, &ToChild::Drop { handle: 3 }).unwrap();

        let mut r = buf.as_slice();
        let first: ToChild = read_frame(&mut r).unwrap().unwrap();
        assert!(
            matches!(first, ToChild::HostReply { call_id: 7, result: Ok(Some(ref s)) } if s == "ok")
        );
        let second: ToChild = read_frame(&mut r).unwrap().unwrap();
        assert!(matches!(second, ToChild::Drop { handle: 3 }));
        assert!(read_frame::<_, ToChild>(&mut r).unwrap().is_none());
    }

    #[test]
    fn errors_cross_the_boundary_with_their_variant() {
        let msg = ToParent::Response {
            id: 1,
            result: Err(crate::Error::Timeout(30)),
        };
        let buf = encode_frame(&msg).unwrap();
        let back: ToParent = read_frame(&mut buf.as_slice()).unwrap().unwrap();
        assert!(matches!(
            back,
            ToParent::Response {
                result: Err(crate::Error::Timeout(30)),
                ..
            }
        ));
    }

    #[test]
    fn oversized_frames_are_rejected_before_allocation() {
        let len = u32::try_from(MAX_FRAME_BYTES + 1).unwrap().to_be_bytes();
        let err = read_frame::<_, ToChild>(&mut len.as_slice()).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn truncated_frames_are_an_error_not_eof() {
        let mut buf = encode_frame(&ToChild::Drop { handle: 1 }).unwrap();
        buf.truncate(buf.len() - 2);
        let err = read_frame::<_, ToChild>(&mut buf.as_slice()).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }
}
