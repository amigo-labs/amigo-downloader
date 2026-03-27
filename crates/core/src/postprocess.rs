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
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self {
            auto_extract: true,
            delete_archives: true,
            verify_checksums: true,
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
pub async fn run_pipeline(download_path: &Path, config: &PostProcessConfig) -> Result<(), crate::Error> {
    if !config.auto_extract {
        return Ok(());
    }

    let archive = match archive_type(download_path) {
        Some(t) => t,
        None => return Ok(()), // Not an archive, nothing to do
    };

    let output_dir = download_path
        .parent()
        .unwrap_or(Path::new("."));

    info!("Post-processing: extracting {:?} ({:?})", download_path, archive);

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
    run_external("unrar", &["x", "-o+", &archive.to_string_lossy(), &format!("{}/", output_dir.to_string_lossy())])
}

fn extract_zip(archive: &Path, output_dir: &Path) -> Result<(), crate::Error> {
    run_external("unzip", &["-o", &archive.to_string_lossy(), "-d", &output_dir.to_string_lossy()])
}

fn extract_7z(archive: &Path, output_dir: &Path) -> Result<(), crate::Error> {
    run_external("7z", &["x", &archive.to_string_lossy(), &format!("-o{}", output_dir.to_string_lossy()), "-y"])
}

fn extract_tar(archive: &Path, output_dir: &Path) -> Result<(), crate::Error> {
    run_external("tar", &["xf", &archive.to_string_lossy(), "-C", &output_dir.to_string_lossy()])
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
        assert!(matches!(archive_type(Path::new("file.rar")), Some(ArchiveType::Rar)));
        assert!(matches!(archive_type(Path::new("file.zip")), Some(ArchiveType::Zip)));
        assert!(matches!(archive_type(Path::new("file.7z")), Some(ArchiveType::SevenZip)));
        assert!(matches!(archive_type(Path::new("file.tar.gz")), Some(ArchiveType::Gzip)));
        assert!(matches!(archive_type(Path::new("file.r00")), Some(ArchiveType::Rar)));
        assert!(matches!(archive_type(Path::new("file.r15")), Some(ArchiveType::Rar)));
        assert!(archive_type(Path::new("file.txt")).is_none());
        assert!(archive_type(Path::new("file.mkv")).is_none());
    }
}
