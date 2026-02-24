//! Filesystem layer management.
//!
//! Each image is composed of ordered layers. Layers are content-addressed
//! by their SHA-256 hash and stored in the local layer cache.

use std::path::Path;

use containust_common::error::{ContainustError, Result};
use containust_common::types::Sha256Hash;

/// A single filesystem layer in an image.
#[derive(Debug, Clone)]
pub struct Layer {
    /// Content-addressed hash of this layer.
    pub hash: Sha256Hash,
    /// Size of the layer in bytes.
    pub size_bytes: u64,
}

/// Extracts a tar archive to the target directory.
///
/// Supports both plain `.tar` and gzip-compressed `.tar.gz` / `.tgz` archives.
///
/// # Errors
///
/// Returns an error if extraction or hash computation fails.
pub fn extract_layer(archive_path: &Path, target: &Path) -> Result<Layer> {
    tracing::info!(
        archive = %archive_path.display(),
        target = %target.display(),
        "extracting layer"
    );

    std::fs::create_dir_all(target).map_err(|e| ContainustError::Io {
        path: target.to_path_buf(),
        source: e,
    })?;

    let file = std::fs::File::open(archive_path).map_err(|e| ContainustError::Io {
        path: archive_path.to_path_buf(),
        source: e,
    })?;

    let metadata = file.metadata().map_err(|e| ContainustError::Io {
        path: archive_path.to_path_buf(),
        source: e,
    })?;
    let size_bytes = metadata.len();

    let is_gzip = is_gzip_archive(archive_path);

    if is_gzip {
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(target).map_err(|e| ContainustError::Io {
            path: target.to_path_buf(),
            source: e,
        })?;
    } else {
        let mut archive = tar::Archive::new(file);
        archive.unpack(target).map_err(|e| ContainustError::Io {
            path: target.to_path_buf(),
            source: e,
        })?;
    }

    let hash = crate::hash::hash_file(archive_path)?;
    tracing::info!(hash = %hash, size = size_bytes, "layer extracted");

    Ok(Layer { hash, size_bytes })
}

/// Determines whether the archive is gzip-compressed based on extension.
fn is_gzip_archive(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("gz") || ext.eq_ignore_ascii_case("tgz"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn create_test_tar(dir: &Path) -> std::path::PathBuf {
        let tar_path = dir.join("test.tar");
        let file = std::fs::File::create(&tar_path).expect("failed to create tar file");
        let mut builder = tar::Builder::new(file);
        let data = b"hello from layer";
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, "hello.txt", &data[..])
            .expect("failed to append data");
        builder.finish().expect("failed to finish tar");
        tar_path
    }

    fn create_test_tar_gz(dir: &Path) -> std::path::PathBuf {
        let tar_gz_path = dir.join("test.tar.gz");
        let file = std::fs::File::create(&tar_gz_path).expect("failed to create tar.gz");
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        let data = b"hello from gzipped layer";
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, "gzhello.txt", &data[..])
            .expect("failed to append data");
        let encoder = builder.into_inner().expect("failed to finish encoder");
        let _ = encoder.finish().expect("failed to finish gzip");
        tar_gz_path
    }

    #[test]
    fn extract_plain_tar_creates_expected_files() {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let tar_path = create_test_tar(dir.path());
        let target = dir.path().join("extracted");

        let layer = extract_layer(&tar_path, &target).expect("extract failed");
        assert!(target.join("hello.txt").exists());
        assert!(layer.size_bytes > 0);

        let content = std::fs::read_to_string(target.join("hello.txt")).expect("read failed");
        assert_eq!(content, "hello from layer");
    }

    #[test]
    fn extract_gzip_tar_creates_expected_files() {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let tar_gz_path = create_test_tar_gz(dir.path());
        let target = dir.path().join("extracted_gz");

        let layer = extract_layer(&tar_gz_path, &target).expect("extract failed");
        assert!(target.join("gzhello.txt").exists());
        assert!(layer.size_bytes > 0);

        let content = std::fs::read_to_string(target.join("gzhello.txt")).expect("read failed");
        assert_eq!(content, "hello from gzipped layer");
    }

    #[test]
    fn extract_nonexistent_archive_returns_error() {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let result = extract_layer(&dir.path().join("missing.tar"), &dir.path().join("out"));
        assert!(result.is_err());
    }

    #[test]
    fn is_gzip_archive_detects_extensions() {
        assert!(is_gzip_archive(Path::new("layer.tar.gz")));
        assert!(is_gzip_archive(Path::new("layer.tgz")));
        assert!(!is_gzip_archive(Path::new("layer.tar")));
        assert!(!is_gzip_archive(Path::new("layer.zip")));
    }
}
