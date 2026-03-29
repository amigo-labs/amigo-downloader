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
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self {
            auto_extract: true,
            delete_archives: true,
            verify_checksums: true,
            par2_repair: true,
            par2_delete_after: true,
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
    run_external(
        "unrar",
        &[
            "x",
            "-o+",
            &archive.to_string_lossy(),
            &format!("{}/", output_dir.to_string_lossy()),
        ],
    )
}

fn extract_zip(archive: &Path, output_dir: &Path) -> Result<(), crate::Error> {
    run_external(
        "unzip",
        &[
            "-o",
            &archive.to_string_lossy(),
            "-d",
            &output_dir.to_string_lossy(),
        ],
    )
}

fn extract_7z(archive: &Path, output_dir: &Path) -> Result<(), crate::Error> {
    run_external(
        "7z",
        &[
            "x",
            &archive.to_string_lossy(),
            &format!("-o{}", output_dir.to_string_lossy()),
            "-y",
        ],
    )
}

fn extract_tar(archive: &Path, output_dir: &Path) -> Result<(), crate::Error> {
    run_external(
        "tar",
        &[
            "xf",
            &archive.to_string_lossy(),
            "-C",
            &output_dir.to_string_lossy(),
        ],
    )
}

/// Run the full Usenet post-processing pipeline for a directory of downloaded files:
/// 1. PAR2 verify/repair (if .par2 files present)
/// 2. Extract archives (RAR, ZIP, 7z)
/// 3. Clean up PAR2 and archive files
pub async fn run_usenet_pipeline(
    download_dir: &Path,
    config: &PostProcessConfig,
) -> Result<(), crate::Error> {
    // Step 1: Find and run PAR2 verify/repair
    if config.par2_repair
        && let Some(par2_file) = find_par2_file(download_dir)
    {
        info!("PAR2 verify: {:?}", par2_file);
        match run_external("par2", &["v", &par2_file.to_string_lossy()]) {
            Ok(()) => info!("PAR2 verification passed"),
            Err(_) => {
                info!("PAR2 verification failed, attempting repair...");
                match run_external("par2", &["r", &par2_file.to_string_lossy()]) {
                    Ok(()) => info!("PAR2 repair successful"),
                    Err(e) => warn!("PAR2 repair failed: {e}"),
                }
            }
        }

        if config.par2_delete_after {
            delete_par2_files(download_dir);
        }
    }

    // Step 2: Extract archives
    if config.auto_extract {
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

        // Delete multipart rar volumes (.r00, .r01, etc.)
        if config.delete_archives {
            delete_rar_volumes(download_dir);
        }
    }

    Ok(())
}

fn find_par2_file(dir: &Path) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    // Prefer the main .par2 file (not .vol*.par2)
    let mut par2_files: Vec<std::path::PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("par2"))
        })
        .collect();
    par2_files.sort_by_key(|p| p.to_string_lossy().len());
    par2_files.into_iter().next()
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
