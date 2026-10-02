//! Local, on-disk cache of compiled QuickJS bytecode for plugins.
//!
//! Loading a plugin costs an SWC transpile (cached in memory by
//! [`crate::transpiler`]) plus a QuickJS parse/compile of the resulting JS.
//! This cache persists the compiled bytecode so subsequent daemon starts skip
//! both.
//!
//! # Security constraints
//!
//! QuickJS bytecode is version-bound and **not validated** before execution:
//! malformed or mismatched bytecode is a memory-safety problem, not a parse
//! error. Therefore:
//!
//! - Bytecode is only ever produced **locally**, by [`compile_script`], from
//!   source that already passed the loader's checks. The registry serves
//!   `.ts`/`.js` source only (`registry::download_plugin` rejects anything
//!   else); bytecode is never accepted from a remote input.
//! - The cache key includes the QuickJS version, the cache format version, the
//!   runtime crate version and the target's pointer width / endianness, so an
//!   engine bump is a cache *miss*, never a load of stale bytecode.
//! - Every entry carries a SHA-256 of its payload; a truncated or corrupted
//!   entry is detected before it reaches `JS_ReadObject`, deleted, and the
//!   loader falls back to source.
//! - The cache directory must only be writable by the daemon. Plugin JS has no
//!   filesystem access at all, so the sandbox cannot reach it.

use std::ffi::{CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use rquickjs::{Ctx, qjs};
use sha2::{Digest, Sha256};
use tracing::{debug, warn};

/// Bump to invalidate every existing entry after a change to the file layout
/// or to how plugin source is wrapped before compilation.
const CACHE_FORMAT_VERSION: u32 = 1;

/// Magic prefix of a cache entry file.
const ENTRY_MAGIC: &[u8; 8] = b"AMQJSBC\x01";

/// File extension of cache entries.
const ENTRY_EXT: &str = "qjsc";

/// Default bound on the number of cached entries.
pub const DEFAULT_MAX_ENTRIES: usize = 256;

/// Upper bound on a cache entry read from disk.
const MAX_ENTRY_BYTES: u64 = 16 * 1024 * 1024;

/// Identity of the engine that produces and consumes the bytecode. Part of
/// every cache key.
static ENGINE_FINGERPRINT: LazyLock<String> = LazyLock::new(|| {
    // SAFETY: JS_GetVersion returns a pointer to a static NUL-terminated string.
    let qjs_version = unsafe { CStr::from_ptr(qjs::JS_GetVersion()) }
        .to_string_lossy()
        .into_owned();
    format!(
        "quickjs-ng={qjs_version};fmt={CACHE_FORMAT_VERSION};runtime={};ptr={};endian={}",
        env!("CARGO_PKG_VERSION"),
        usize::BITS,
        if cfg!(target_endian = "little") {
            "le"
        } else {
            "be"
        },
    )
});

/// The engine fingerprint used in cache keys (exposed for diagnostics/tests).
pub fn engine_fingerprint() -> &'static str {
    &ENGINE_FINGERPRINT
}

/// On-disk bytecode cache.
#[derive(Debug, Clone)]
pub struct BytecodeCache {
    dir: PathBuf,
    max_entries: usize,
    fingerprint: String,
}

impl BytecodeCache {
    /// Open (creating if needed) a cache in `dir`, pruning it to
    /// `max_entries`.
    pub fn open(dir: impl Into<PathBuf>, max_entries: usize) -> std::io::Result<Self> {
        let cache = Self {
            dir: dir.into(),
            max_entries: max_entries.max(1),
            fingerprint: engine_fingerprint().to_string(),
        };
        create_private_dir(&cache.dir)?;
        cache.prune();
        Ok(cache)
    }

    /// Same cache, but keyed with a different engine fingerprint. Only for
    /// tests that simulate an engine upgrade.
    #[doc(hidden)]
    pub fn with_fingerprint_for_test(mut self, fingerprint: &str) -> Self {
        self.fingerprint = fingerprint.to_string();
        self
    }

    /// Cache key for `source` compiled under `filename`.
    pub fn key(&self, filename: &str, source: &str) -> String {
        let mut h = Sha256::new();
        h.update(self.fingerprint.as_bytes());
        h.update([0u8]);
        h.update(filename.as_bytes());
        h.update([0u8]);
        h.update(source.as_bytes());
        hex::encode(h.finalize())
    }

    fn entry_path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.{ENTRY_EXT}"))
    }

    /// Return the bytecode stored under `key`, if present and intact. A
    /// damaged entry is deleted and reported as a miss.
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        let path = self.entry_path(key);
        let meta = std::fs::metadata(&path).ok()?;
        if meta.len() > MAX_ENTRY_BYTES {
            self.discard(&path, "oversized");
            return None;
        }
        let data = std::fs::read(&path).ok()?;
        match decode_entry(&data) {
            Some(payload) => {
                // Refresh mtime so pruning evicts least-recently-used entries.
                if let Ok(f) = std::fs::File::options().write(true).open(&path) {
                    let _ = f.set_modified(std::time::SystemTime::now());
                }
                Some(payload.to_vec())
            }
            None => {
                self.discard(&path, "corrupted");
                None
            }
        }
    }

    /// Store `bytecode` under `key` (atomic write-then-rename), then prune.
    pub fn put(&self, key: &str, bytecode: &[u8]) {
        let path = self.entry_path(key);
        let tmp = self.dir.join(format!("{key}.{ENTRY_EXT}.tmp"));
        let result = std::fs::write(&tmp, encode_entry(bytecode))
            .and_then(|()| std::fs::rename(&tmp, &path));
        if let Err(e) = result {
            debug!("bytecode cache: failed to write {}: {e}", path.display());
            let _ = std::fs::remove_file(&tmp);
            return;
        }
        self.prune();
    }

    /// Delete a damaged entry so the next load recompiles from source.
    pub fn remove(&self, key: &str) {
        let _ = std::fs::remove_file(self.entry_path(key));
    }

    fn discard(&self, path: &Path, why: &str) {
        warn!("bytecode cache: discarding {why} entry {}", path.display());
        let _ = std::fs::remove_file(path);
    }

    /// Evict the least-recently-used entries beyond `max_entries`.
    fn prune(&self) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
            .flatten()
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == ENTRY_EXT || ext == "tmp")
            })
            .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
            .collect();
        if files.len() <= self.max_entries {
            return;
        }
        files.sort_by_key(|(mtime, _)| *mtime);
        let excess = files.len() - self.max_entries;
        for (_, path) in files.into_iter().take(excess) {
            let _ = std::fs::remove_file(path);
        }
    }

    /// Number of entries currently on disk.
    pub fn len(&self) -> usize {
        std::fs::read_dir(&self.dir)
            .map(|it| {
                it.flatten()
                    .filter(|e| e.path().extension().is_some_and(|ext| ext == ENTRY_EXT))
                    .count()
            })
            .unwrap_or(0)
    }

    /// Whether the cache holds no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// `MAGIC || sha256(payload) || payload`
fn encode_entry(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(ENTRY_MAGIC.len() + 32 + payload.len());
    out.extend_from_slice(ENTRY_MAGIC);
    out.extend_from_slice(&Sha256::digest(payload));
    out.extend_from_slice(payload);
    out
}

fn decode_entry(data: &[u8]) -> Option<&[u8]> {
    let rest = data.strip_prefix(ENTRY_MAGIC.as_slice())?;
    if rest.len() < 32 {
        return None;
    }
    let (digest, payload) = rest.split_at(32);
    if payload.is_empty() || Sha256::digest(payload).as_slice() != digest {
        return None;
    }
    Some(payload)
}

/// Take the pending exception off `ctx` and render it as a message.
fn take_exception(ctx: &Ctx<'_>) -> String {
    crate::engine::describe_js_error(ctx, rquickjs::Error::Exception)
}

/// Compile `source` as a global (non-module) script and serialize the
/// resulting function to bytecode, without running it.
pub(crate) fn compile_script(
    ctx: &Ctx<'_>,
    source: &str,
    filename: &str,
) -> Result<Vec<u8>, String> {
    let src = CString::new(source).map_err(|_| "source contains a NUL byte".to_string())?;
    let name = CString::new(filename).map_err(|_| "filename contains a NUL byte".to_string())?;
    let raw = ctx.as_raw().as_ptr();
    // SAFETY: `raw` is the live context we are inside of; `src`/`name` are
    // NUL-terminated and outlive the call. Every JSValue we receive is freed
    // exactly once, and the buffer from JS_WriteObject is released with
    // js_free on the same context.
    unsafe {
        let func = qjs::JS_Eval(
            raw,
            src.as_ptr(),
            source.len() as _,
            name.as_ptr(),
            (qjs::JS_EVAL_TYPE_GLOBAL | qjs::JS_EVAL_FLAG_COMPILE_ONLY) as i32,
        );
        if qjs::JS_IsException(func) {
            return Err(take_exception(ctx));
        }
        let mut len = 0usize;
        let buf = qjs::JS_WriteObject(
            raw,
            &mut len as *mut usize as *mut _,
            func,
            qjs::JS_WRITE_OBJ_BYTECODE as i32,
        );
        qjs::JS_FreeValue(raw, func);
        if buf.is_null() {
            return Err(take_exception(ctx));
        }
        let bytes = std::slice::from_raw_parts(buf, len).to_vec();
        qjs::js_free(raw, buf as *mut _);
        Ok(bytes)
    }
}

/// Deserialize and run bytecode produced by [`compile_script`].
///
/// # Safety
/// QuickJS trusts its input. `bytecode` must have been produced by
/// [`compile_script`] in this same engine build, and must be intact.
pub(crate) unsafe fn eval_script_bytecode(ctx: &Ctx<'_>, bytecode: &[u8]) -> Result<(), String> {
    let raw = ctx.as_raw().as_ptr();
    // SAFETY: see function contract; values are freed exactly once
    // (JS_EvalFunction consumes `func`).
    unsafe {
        let func = qjs::JS_ReadObject(
            raw,
            bytecode.as_ptr(),
            bytecode.len() as _,
            qjs::JS_READ_OBJ_BYTECODE as i32,
        );
        if qjs::JS_IsException(func) {
            return Err(take_exception(ctx));
        }
        let result = qjs::JS_EvalFunction(raw, func);
        if qjs::JS_IsException(result) {
            return Err(take_exception(ctx));
        }
        qjs::JS_FreeValue(raw, result);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_cache(name: &str, max: usize) -> BytecodeCache {
        let dir = std::env::temp_dir().join(format!("amigo-bc-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        BytecodeCache::open(dir, max).unwrap()
    }

    #[test]
    fn round_trip_and_corruption_detection() {
        let cache = tmp_cache("rt", 8);
        let key = cache.key("plugin.js", "var x = 1;");
        assert!(cache.get(&key).is_none());
        cache.put(&key, b"bytecode-bytes");
        assert_eq!(cache.get(&key).as_deref(), Some(&b"bytecode-bytes"[..]));

        // Truncate the entry: it must read as a miss and be deleted.
        let path = cache.entry_path(&key);
        let data = std::fs::read(&path).unwrap();
        std::fs::write(&path, &data[..data.len() - 3]).unwrap();
        assert!(cache.get(&key).is_none());
        assert!(!path.exists(), "corrupted entry must be removed");
    }

    #[test]
    fn engine_version_is_part_of_the_key() {
        let cache = tmp_cache("ver", 8);
        let bumped = cache.clone().with_fingerprint_for_test("quickjs-ng=99.0.0");
        assert_ne!(cache.key("p.js", "src"), bumped.key("p.js", "src"));
        assert!(engine_fingerprint().contains("quickjs-ng="));
    }

    #[test]
    fn prune_keeps_at_most_max_entries() {
        let cache = tmp_cache("prune", 3);
        for i in 0..6 {
            let key = cache.key("p.js", &i.to_string());
            cache.put(&key, b"x");
        }
        assert!(cache.len() <= 3, "len = {}", cache.len());
    }
}
