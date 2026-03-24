use std::path::Path;

use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::provider::ArchiveFormat;

/// Download a file from a URL to a destination path with optional progress bar.
///
/// Returns the SHA256 hex digest of the downloaded file.
pub async fn download_file(url: &str, dest: &Path, quiet: bool) -> Result<String, DownloadError> {
    let response = reqwest::get(url).await.map_err(|e| DownloadError::Http {
        url: url.to_string(),
        source: e,
    })?;

    if !response.status().is_success() {
        return Err(DownloadError::HttpStatus {
            url: url.to_string(),
            status: response.status().as_u16(),
        });
    }

    let total_size = response.content_length();

    let pb = if quiet {
        ProgressBar::hidden()
    } else {
        match total_size {
            Some(size) => {
                let pb = ProgressBar::new(size);
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template(
                            "{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})",
                        )
                        .expect("valid template")
                        .progress_chars("#>-"),
                );
                pb
            }
            None => {
                let pb = ProgressBar::new_spinner();
                pb.set_style(
                    ProgressStyle::default_spinner()
                        .template("{spinner:.green} {bytes} downloaded")
                        .expect("valid template"),
                );
                pb
            }
        }
    };

    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| DownloadError::Io {
            path: dest.to_path_buf(),
            source: e,
        })?;

    let mut hasher = Sha256::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| DownloadError::Http {
            url: url.to_string(),
            source: e,
        })?;
        file.write_all(&chunk)
            .await
            .map_err(|e| DownloadError::Io {
                path: dest.to_path_buf(),
                source: e,
            })?;
        hasher.update(&chunk);
        pb.inc(chunk.len() as u64);
    }

    file.flush().await.map_err(|e| DownloadError::Io {
        path: dest.to_path_buf(),
        source: e,
    })?;

    pb.finish_and_clear();

    let hash = format!("{:x}", hasher.finalize());
    Ok(hash)
}

/// Verify a SHA256 checksum against an expected value.
pub fn verify_checksum(actual: &str, expected: &str) -> Result<(), DownloadError> {
    if actual != expected {
        return Err(DownloadError::ChecksumMismatch {
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

/// Extract an archive to a destination directory.
///
/// Supports `.tar.gz`, `.tar.xz`, and `.zip` formats.
pub fn extract_archive(
    archive_path: &Path,
    dest_dir: &Path,
    format: ArchiveFormat,
) -> Result<(), DownloadError> {
    match format {
        ArchiveFormat::TarGz => extract_tar_gz(archive_path, dest_dir),
        ArchiveFormat::TarXz => extract_tar_xz(archive_path, dest_dir),
        ArchiveFormat::Zip => extract_zip(archive_path, dest_dir),
    }
}

fn extract_tar_gz(archive_path: &Path, dest_dir: &Path) -> Result<(), DownloadError> {
    let file = std::fs::File::open(archive_path).map_err(|e| DownloadError::Io {
        path: archive_path.to_path_buf(),
        source: e,
    })?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);
    archive
        .unpack(dest_dir)
        .map_err(|e| DownloadError::Extraction {
            path: archive_path.to_path_buf(),
            reason: e.to_string(),
        })?;
    Ok(())
}

fn extract_tar_xz(archive_path: &Path, dest_dir: &Path) -> Result<(), DownloadError> {
    // xz2 crate would be needed for .tar.xz; for now use a subprocess fallback
    // since xz2 adds significant compile time and tar.xz is less common.
    let status = std::process::Command::new("tar")
        .args(["xf", &archive_path.to_string_lossy(), "-C"])
        .arg(dest_dir)
        .status()
        .map_err(|e| DownloadError::Extraction {
            path: archive_path.to_path_buf(),
            reason: format!("failed to run tar: {e}"),
        })?;

    if !status.success() {
        return Err(DownloadError::Extraction {
            path: archive_path.to_path_buf(),
            reason: format!("tar exited with status {status}"),
        });
    }
    Ok(())
}

fn extract_zip(archive_path: &Path, dest_dir: &Path) -> Result<(), DownloadError> {
    let file = std::fs::File::open(archive_path).map_err(|e| DownloadError::Io {
        path: archive_path.to_path_buf(),
        source: e,
    })?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| DownloadError::Extraction {
        path: archive_path.to_path_buf(),
        reason: e.to_string(),
    })?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| DownloadError::Extraction {
            path: archive_path.to_path_buf(),
            reason: e.to_string(),
        })?;

        let out_path = dest_dir.join(entry.mangled_name());

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| DownloadError::Io {
                path: out_path.clone(),
                source: e,
            })?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| DownloadError::Io {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
            let mut outfile = std::fs::File::create(&out_path).map_err(|e| DownloadError::Io {
                path: out_path.clone(),
                source: e,
            })?;
            std::io::copy(&mut entry, &mut outfile).map_err(|e| DownloadError::Io {
                path: out_path.clone(),
                source: e,
            })?;

            // Preserve Unix permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = entry.unix_mode() {
                    std::fs::set_permissions(&out_path, std::fs::Permissions::from_mode(mode))
                        .map_err(|e| DownloadError::Io {
                            path: out_path,
                            source: e,
                        })?;
                }
            }
        }
    }

    Ok(())
}

/// Download a tool archive, verify its checksum, and extract it to the cache.
///
/// Uses atomic writes: downloads to a temp directory first, then renames
/// to the final cache path to avoid partial/corrupt cache entries.
pub async fn download_and_install(
    url: &str,
    install_dir: &Path,
    format: ArchiveFormat,
    expected_checksum: Option<&str>,
    quiet: bool,
) -> Result<(), DownloadError> {
    let temp_dir = tempfile::tempdir().map_err(|e| DownloadError::Io {
        path: std::env::temp_dir(),
        source: e,
    })?;

    // Determine archive filename from URL
    let archive_name = url.rsplit('/').next().unwrap_or("archive.tar.gz");
    let archive_path = temp_dir.path().join(archive_name);

    // Download
    let checksum = download_file(url, &archive_path, quiet).await?;

    // Verify checksum if provided
    if let Some(expected) = expected_checksum {
        verify_checksum(&checksum, expected)?;
    }

    // Run blocking extraction and filesystem ops off the async executor
    let archive_path_owned = archive_path.clone();
    let staging_dir = temp_dir.path().join("staging");
    let install_dir_owned = install_dir.to_path_buf();

    tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&staging_dir).map_err(|e| DownloadError::Io {
            path: staging_dir.clone(),
            source: e,
        })?;

        extract_archive(&archive_path_owned, &staging_dir, format)?;

        // Ensure parent exists
        if let Some(parent) = install_dir_owned.parent() {
            std::fs::create_dir_all(parent).map_err(|e| DownloadError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        // Atomic install: remove existing, then rename staging into place.
        // If rename fails (cross-device), fall back to recursive copy.
        // Clean up install_dir on failure to avoid partial installs.
        if install_dir_owned.exists() {
            std::fs::remove_dir_all(&install_dir_owned).map_err(|e| DownloadError::Io {
                path: install_dir_owned.clone(),
                source: e,
            })?;
        }

        if std::fs::rename(&staging_dir, &install_dir_owned).is_err()
            && let Err(e) = copy_dir_recursive(&staging_dir, &install_dir_owned)
        {
            // Clean up partial install on failure
            let _ = std::fs::remove_dir_all(&install_dir_owned);
            return Err(e);
        }

        Ok(())
    })
    .await
    .map_err(|e| DownloadError::Extraction {
        path: install_dir.to_path_buf(),
        reason: format!("blocking task failed: {e}"),
    })??;

    Ok(())
}

/// Recursively copy a directory tree.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), DownloadError> {
    std::fs::create_dir_all(dst).map_err(|e| DownloadError::Io {
        path: dst.to_path_buf(),
        source: e,
    })?;

    for entry in std::fs::read_dir(src).map_err(|e| DownloadError::Io {
        path: src.to_path_buf(),
        source: e,
    })? {
        let entry = entry.map_err(|e| DownloadError::Io {
            path: src.to_path_buf(),
            source: e,
        })?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path).map_err(|e| DownloadError::Io {
                path: dst_path,
                source: e,
            })?;
        }
    }
    Ok(())
}

/// Errors that occur during download and extraction operations.
#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("HTTP request failed for `{url}`: {source}")]
    Http { url: String, source: reqwest::Error },

    #[error("HTTP {status} error for `{url}`")]
    HttpStatus { url: String, status: u16 },

    #[error("I/O error at `{}`: {source}", path.display())]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    #[error("failed to extract `{}`: {reason}", path.display())]
    Extraction {
        path: std::path::PathBuf,
        reason: String,
    },

    #[error("checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_checksum_match() {
        assert!(verify_checksum("abc123", "abc123").is_ok());
    }

    #[test]
    fn test_verify_checksum_mismatch() {
        let err = verify_checksum("abc123", "def456").unwrap_err();
        assert!(err.to_string().contains("checksum mismatch"));
    }

    #[test]
    fn test_extract_tar_gz() {
        // Create a small tar.gz in memory for testing
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("test.tar.gz");
        let extract_dir = dir.path().join("extracted");
        std::fs::create_dir_all(&extract_dir).unwrap();

        // Create a tar.gz with a single file
        let src_file = dir.path().join("src_test.txt");
        std::fs::write(&src_file, "hello world").unwrap();

        let file = std::fs::File::create(&archive_path).unwrap();
        let gz = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(gz);
        builder
            .append_path_with_name(&src_file, "test.txt")
            .unwrap();
        let gz = builder.into_inner().unwrap();
        gz.finish().unwrap();

        // Extract
        extract_archive(&archive_path, &extract_dir, ArchiveFormat::TarGz).unwrap();
        assert!(extract_dir.join("test.txt").exists());

        let extracted_content = std::fs::read_to_string(extract_dir.join("test.txt")).unwrap();
        assert_eq!(extracted_content, "hello world");
    }

    #[test]
    fn test_extract_zip() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("test.zip");
        let extract_dir = dir.path().join("extracted");
        std::fs::create_dir_all(&extract_dir).unwrap();

        // Create a zip with a single file
        let file = std::fs::File::create(&archive_path).unwrap();
        let mut zip_writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip_writer.start_file("test.txt", options).unwrap();
        std::io::Write::write_all(&mut zip_writer, b"hello zip").unwrap();
        zip_writer.finish().unwrap();

        // Extract
        extract_archive(&archive_path, &extract_dir, ArchiveFormat::Zip).unwrap();
        assert!(extract_dir.join("test.txt").exists());

        let content = std::fs::read_to_string(extract_dir.join("test.txt")).unwrap();
        assert_eq!(content, "hello zip");
    }

    #[test]
    fn test_download_error_display() {
        let err = DownloadError::HttpStatus {
            url: "https://example.com/file.tar.gz".to_string(),
            status: 404,
        };
        assert_eq!(
            err.to_string(),
            "HTTP 404 error for `https://example.com/file.tar.gz`"
        );

        let err = DownloadError::ChecksumMismatch {
            expected: "aaa".to_string(),
            actual: "bbb".to_string(),
        };
        assert!(err.to_string().contains("checksum mismatch"));
    }

    #[test]
    fn test_copy_dir_recursive() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");

        std::fs::create_dir_all(src.join("subdir")).unwrap();
        std::fs::write(src.join("file.txt"), "root file").unwrap();
        std::fs::write(src.join("subdir/nested.txt"), "nested file").unwrap();

        copy_dir_recursive(&src, &dst).unwrap();

        assert!(dst.join("file.txt").exists());
        assert!(dst.join("subdir/nested.txt").exists());
        assert_eq!(
            std::fs::read_to_string(dst.join("file.txt")).unwrap(),
            "root file"
        );
        assert_eq!(
            std::fs::read_to_string(dst.join("subdir/nested.txt")).unwrap(),
            "nested file"
        );
    }

    #[tokio::test]
    async fn test_download_file_invalid_url() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("output");
        let result = download_file("http://localhost:1/nonexistent", &dest, true).await;
        assert!(result.is_err());
    }
}
