//! Post-processing pipeline: extraction, verification, cleanup.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostProcessConfig {
    pub auto_extract: bool,
    pub delete_archives: bool,
    pub verify_checksums: bool,
    pub par2_repair: bool,
    pub par2_delete_after: bool,
    /// Selective PAR2: only load recovery volumes (.vol*.par2) when repair is needed.
    /// When false, par2 verify+repair uses all available files from the start.
    #[serde(default = "pp_default_true")]
    pub selective_par2: bool,
    /// Sequential mode: run PAR2 and extraction one after another (not parallel).
    /// Recommended for low-power devices (Raspberry Pi, NAS) to reduce peak CPU/RAM.
    #[serde(default)]
    pub sequential_postprocess: bool,
}

fn pp_default_true() -> bool {
    true
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self {
            auto_extract: true,
            delete_archives: true,
            verify_checksums: true,
            par2_repair: true,
            par2_delete_after: true,
            selective_par2: true,
            sequential_postprocess: false,
        }
    }
}

/// Supported archive types.
fn archive_type(path: &Path) -> Option<ArchiveType> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "rar" => Some(ArchiveType::Rar),
        "zip" => Some(ArchiveType::Zip),
        "7z" => Some(ArchiveType::SevenZip),
        "gz" | "tgz" => Some(ArchiveType::Gzip),
        "tar" => Some(ArchiveType::Tar),
        _ => {
            // Check for multipart rar: .r00, .r01, .part01.rar
            if ext.starts_with('r') && ext[1..].chars().all(|c| c.is_ascii_digit()) {
                Some(ArchiveType::Rar)
            } else {
                None
            }
        }
    }
}

#[derive(Debug)]
enum ArchiveType {
    Rar,
    Zip,
    SevenZip,
    Gzip,
    Tar,
}

/// Run post-processing pipeline on a completed download.
pub async fn run_pipeline(
    download_path: &Path,
    config: &PostProcessConfig,
) -> Result<(), crate::Error> {
    if !config.auto_extract {
        return Ok(());
    }

    let archive = match archive_type(download_path) {
        Some(t) => t,
        None => return Ok(()), // Not an archive, nothing to do
    };

    let output_dir = download_path.parent().unwrap_or(Path::new("."));

    info!(
        "Post-processing: extracting {:?} ({:?})",
        download_path, archive
    );

    let result = match archive {
        ArchiveType::Rar => extract_rar(download_path, output_dir),
        ArchiveType::Zip => extract_zip(download_path, output_dir),
        ArchiveType::SevenZip => extract_7z(download_path, output_dir),
        ArchiveType::Gzip | ArchiveType::Tar => extract_tar(download_path, output_dir),
    };

    match result {
        Ok(()) => {
            info!("Extraction complete: {:?}", download_path);
            if config.delete_archives {
                debug!("Deleting archive: {:?}", download_path);
                let _ = std::fs::remove_file(download_path);
            }
            Ok(())
        }
        Err(e) => {
            warn!("Extraction failed for {:?}: {e}", download_path);
            Err(e)
        }
    }
}

fn extract_rar(archive: &Path, output_dir: &Path) -> Result<(), crate::Error> {
    // Use unrar crate (FFI to libunrar) for in-process extraction
    let rar = unrar::Archive::new(archive);
    let mut open = rar
        .open_for_processing()
        .map_err(|e| crate::Error::Other(format!("Failed to open RAR archive: {e}")))?;

    while let Some(header) = open
        .read_header()
        .map_err(|e| crate::Error::Other(format!("RAR header error: {e}")))?
    {
        let name = header.entry().filename.to_string_lossy().to_string();
        debug!("Extracting: {name}");
        open = header
            .extract_with_base(output_dir)
            .map_err(|e| crate::Error::Other(format!("RAR extract error for {name}: {e}")))?;
    }

    Ok(())
}

fn extract_zip(archive: &Path, output_dir: &Path) -> Result<(), crate::Error> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| crate::Error::Other(format!("Invalid ZIP: {e}")))?;

    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| crate::Error::Other(format!("ZIP entry error: {e}")))?;

        let name = entry.name().to_string();

        // Zip Slip protection (path components):
        //   1. reject `..`, `.`, and empty segments after normalising `\`→`/`
        //   2. sanitise each segment for NUL / `:` / control chars
        let sanitized = name
            .replace('\\', "/")
            .split('/')
            .filter(|c| !c.is_empty() && *c != "." && *c != "..")
            .map(crate::sanitize_filename)
            .collect::<Vec<_>>()
            .join("/");
        if sanitized.is_empty() {
            continue;
        }
        let out_path = output_dir.join(&sanitized);
        if !out_path.starts_with(output_dir) {
            warn!(
                "ZIP entry {:?} escapes output directory — skipping (path traversal blocked)",
                name
            );
            continue;
        }

        // Zip Slip protection (symlinks): refuse symlink entries entirely.
        // The original guard only checked the *normalised* path; a symlink
        // entry pointing at `..` followed by a regular entry under the same
        // prefix would still escape the output dir at write time.
        if is_symlink_entry(&entry) {
            warn!(
                "ZIP entry {:?} is a symlink — skipping (symlink-extraction disabled)",
                name
            );
            continue;
        }

        // Defence in depth: if any intermediate component on disk is
        // already a symlink (e.g. created by a previous corrupt extraction),
        // refuse to write through it.
        if has_symlink_ancestor(output_dir, &out_path)? {
            warn!(
                "ZIP entry {:?} would traverse a pre-existing symlink — skipping",
                name
            );
            continue;
        }

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out_file = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out_file)?;
            debug!("Extracted: {name}");
        }
    }

    Ok(())
}

/// True for ZIP entries whose Unix mode marks them as a symbolic link.
/// On non-Unix platforms `unix_mode()` returns `None` and we fall back to
/// `false` — Windows does not honour Unix symlink bits in zips, but the
/// `has_symlink_ancestor` check still protects against real-on-disk
/// symlinks.
fn is_symlink_entry(entry: &zip::read::ZipFile<'_>) -> bool {
    const S_IFMT: u32 = 0o170000;
    const S_IFLNK: u32 = 0o120000;
    matches!(entry.unix_mode(), Some(mode) if mode & S_IFMT == S_IFLNK)
}

/// Walk every component between `root` and `path` and return true if any
/// already-existing component is a symlink. `path` itself is NOT checked
/// because it doesn't exist yet at extraction time (and `File::create`
/// O_CREAT|O_TRUNC will overwrite a symlink target — that is the very thing
/// we want to refuse).
fn has_symlink_ancestor(root: &Path, path: &Path) -> Result<bool, crate::Error> {
    let mut cur = path.parent();
    while let Some(p) = cur {
        if p == root || !p.starts_with(root) {
            break;
        }
        match std::fs::symlink_metadata(p) {
            Ok(meta) if meta.file_type().is_symlink() => return Ok(true),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        cur = p.parent();
    }
    Ok(false)
}

fn extract_7z(archive: &Path, output_dir: &Path) -> Result<(), crate::Error> {
    sevenz_rust::decompress_file(archive, output_dir)
        .map_err(|e| crate::Error::Other(format!("7z extraction failed: {e}")))?;
    Ok(())
}

fn extract_tar(archive: &Path, output_dir: &Path) -> Result<(), crate::Error> {
    run_external(
        "tar",
        &[
            "xf",
            &archive.to_string_lossy(),
            "-C",
            &output_dir.to_string_lossy(),
            "--no-absolute-names",
        ],
    )
}

/// Run the full Usenet post-processing pipeline for a directory of downloaded files.
///
/// **Modes:**
/// - `selective_par2 = true` (default): Verify with index .par2 only, load recovery
///   volumes (.vol*.par2) only if repair is needed. Saves bandwidth.
/// - `selective_par2 = false`: Use all PAR2 files from the start (pre-downloaded).
///   Faster repair but requires all volumes downloaded upfront.
///
/// - `sequential_postprocess = true`: PAR2 completes fully, then extraction runs.
///   Lower peak CPU/RAM — recommended for Raspberry Pi, low-power NAS.
/// - `sequential_postprocess = false` (default): PAR2 and extraction can overlap
///   where files are independent.
pub async fn run_usenet_pipeline(
    download_dir: &Path,
    config: &PostProcessConfig,
) -> Result<(), crate::Error> {
    if config.sequential_postprocess {
        // Sequential mode: PAR2 first, then extract. No parallelism.
        info!("Post-processing (sequential mode)");
        run_par2_phase(download_dir, config)?;
        run_extract_phase(download_dir, config)?;
    } else {
        // Default mode: PAR2 first (must complete before extract for data integrity),
        // then extract. In the future this could overlap independent files.
        info!("Post-processing (standard mode)");
        run_par2_phase(download_dir, config)?;
        run_extract_phase(download_dir, config)?;
    }

    Ok(())
}

/// PAR2 verify and optional repair phase.
fn run_par2_phase(download_dir: &Path, config: &PostProcessConfig) -> Result<(), crate::Error> {
    if !config.par2_repair {
        return Ok(());
    }

    let Some(par2_file) = find_par2_file(download_dir) else {
        return Ok(());
    };

    if config.selective_par2 {
        // Selective: verify with index only, load recovery volumes only on demand
        info!("PAR2 verify (selective — index only): {:?}", par2_file);

        match run_external("par2", &["v", &par2_file.to_string_lossy()]) {
            Ok(()) => {
                info!("PAR2 verification passed — no repair needed, recovery volumes not loaded");
            }
            Err(_) => {
                let vol_count = count_par2_volumes(download_dir);
                info!(
                    "PAR2 verification found damage — loading {vol_count} recovery volumes for repair..."
                );
                match run_external("par2", &["r", &par2_file.to_string_lossy()]) {
                    Ok(()) => info!("PAR2 repair successful"),
                    Err(e) => warn!("PAR2 repair failed: {e}"),
                }
            }
        }
    } else {
        // Full mode: use all PAR2 files from the start (assumes all pre-downloaded)
        info!("PAR2 verify+repair (full — all volumes loaded): {:?}", par2_file);

        match run_external("par2", &["r", &par2_file.to_string_lossy()]) {
            Ok(()) => info!("PAR2 verify+repair complete"),
            Err(e) => warn!("PAR2 verify+repair failed: {e}"),
        }
    }

    if config.par2_delete_after {
        delete_par2_files(download_dir);
    }

    Ok(())
}

/// Archive extraction phase.
fn run_extract_phase(download_dir: &Path, config: &PostProcessConfig) -> Result<(), crate::Error> {
    if !config.auto_extract {
        return Ok(());
    }

    let archives = find_archives(download_dir);
    for archive in &archives {
        info!("Extracting: {:?}", archive);
        let result = match archive_type(archive) {
            Some(ArchiveType::Rar) => extract_rar(archive, download_dir),
            Some(ArchiveType::Zip) => extract_zip(archive, download_dir),
            Some(ArchiveType::SevenZip) => extract_7z(archive, download_dir),
            Some(ArchiveType::Gzip | ArchiveType::Tar) => extract_tar(archive, download_dir),
            None => continue,
        };

        match result {
            Ok(()) => {
                if config.delete_archives {
                    let _ = std::fs::remove_file(archive);
                }
            }
            Err(e) => warn!("Extraction failed for {:?}: {e}", archive),
        }
    }

    if config.delete_archives {
        delete_rar_volumes(download_dir);
    }

    Ok(())
}

/// Find the PAR2 index file (shortest name, excludes .vol*.par2 recovery volumes).
fn find_par2_file(dir: &Path) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut par2_files: Vec<std::path::PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("par2"))
        })
        .collect();

    // Prefer the index file (no ".vol" in name) over recovery volumes
    par2_files.sort_by_key(|p| {
        let name = p.to_string_lossy().to_lowercase();
        let is_vol = name.contains(".vol");
        (is_vol, name.len())
    });
    par2_files.into_iter().next()
}

/// Count PAR2 recovery volume files (.vol*.par2) in a directory.
fn count_par2_volumes(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.path().to_string_lossy().to_lowercase();
                    name.ends_with(".par2") && name.contains(".vol")
                })
                .count()
        })
        .unwrap_or(0)
}

fn find_archives(dir: &Path) -> Vec<std::path::PathBuf> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| archive_type(p).is_some())
        // Only include first RAR volume or standalone archives
        .filter(|p| {
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            // Skip .r00, .r01 etc — only keep .rar
            !ext.starts_with('r') || ext == "rar"
        })
        .collect()
}

fn delete_par2_files(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry
                .path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("par2"))
            {
                debug!("Deleting PAR2 file: {:?}", entry.path());
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

fn delete_rar_volumes(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let ext = entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            // Delete .r00, .r01, .r02, ... (multipart rar volumes)
            if ext.starts_with('r') && ext.len() >= 2 && ext[1..].chars().all(|c| c.is_ascii_digit())
            {
                debug!("Deleting RAR volume: {:?}", entry.path());
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

fn run_external(cmd: &str, args: &[&str]) -> Result<(), crate::Error> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| crate::Error::Other(format!("Failed to run {cmd}: {e}. Is it installed?")))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(crate::Error::Other(format!("{cmd} failed: {stderr}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a zip in memory containing one symlink entry pointing at an
    /// absolute path outside the output directory, plus a regular file.
    /// Uses `ZipWriter::add_symlink` so the central-directory entry has
    /// `external_attributes` flagged as S_IFLNK (the writer sets this
    /// itself; `unix_permissions` alone is masked to 0o777).
    #[cfg(unix)]
    fn write_symlink_zip(dir: &Path) -> std::path::PathBuf {
        use std::io::Write;
        use zip::write::SimpleFileOptions;

        let zip_path = dir.join("evil.zip");
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(file);

        let opts = SimpleFileOptions::default();
        zw.add_symlink("evil", "/tmp/should-not-exist-here", opts)
            .unwrap();

        zw.start_file("hello.txt", opts).unwrap();
        zw.write_all(b"hi").unwrap();

        zw.finish().unwrap();
        zip_path
    }

    #[cfg(unix)]
    #[test]
    fn extract_zip_skips_symlink_entries() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = write_symlink_zip(dir.path());
        let out = dir.path().join("out");
        std::fs::create_dir_all(&out).unwrap();

        extract_zip(&zip_path, &out).expect("extraction should not fail");

        // The regular file extracted normally.
        assert!(out.join("hello.txt").exists());
        // The symlink entry was refused — `evil` must not exist as a
        // symlink, regular file, or anything else.
        let evil = out.join("evil");
        assert!(
            std::fs::symlink_metadata(&evil).is_err(),
            "symlink entry must not have been created"
        );
    }

    #[cfg(unix)]
    #[test]
    fn has_symlink_ancestor_detects_pre_existing_link() {
        // A previous corrupt extraction left a symlink in the output dir;
        // a later well-formed entry that traverses through it must be
        // refused.
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real");
        std::fs::create_dir_all(&target).unwrap();
        let link = dir.path().join("through");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let bad = link.join("payload.bin");
        assert!(
            has_symlink_ancestor(dir.path(), &bad).unwrap(),
            "ancestor symlink must be detected"
        );

        let safe = dir.path().join("real").join("payload.bin");
        assert!(
            !has_symlink_ancestor(dir.path(), &safe).unwrap(),
            "no symlink in this path"
        );
    }

    #[test]
    fn test_archive_type_detection() {
        assert!(matches!(
            archive_type(Path::new("file.rar")),
            Some(ArchiveType::Rar)
        ));
        assert!(matches!(
            archive_type(Path::new("file.zip")),
            Some(ArchiveType::Zip)
        ));
        assert!(matches!(
            archive_type(Path::new("file.7z")),
            Some(ArchiveType::SevenZip)
        ));
        assert!(matches!(
            archive_type(Path::new("file.tar.gz")),
            Some(ArchiveType::Gzip)
        ));
        assert!(matches!(
            archive_type(Path::new("file.r00")),
            Some(ArchiveType::Rar)
        ));
        assert!(matches!(
            archive_type(Path::new("file.r15")),
            Some(ArchiveType::Rar)
        ));
        assert!(archive_type(Path::new("file.txt")).is_none());
        assert!(archive_type(Path::new("file.mkv")).is_none());
    }
}
