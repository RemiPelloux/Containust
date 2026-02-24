//! Image source protocol handlers.
//!
//! Supports `file://` (local directory), `tar://` (archive), and
//! remote sources with SHA-256 validation. Local-first by design.

use std::path::PathBuf;

use containust_common::error::{ContainustError, Result};

/// Supported image source protocols.
#[derive(Debug, Clone)]
pub enum ImageSource {
    /// Local directory (`file:///path/to/rootfs`).
    File(PathBuf),
    /// Local tar archive (`tar:///path/to/image.tar`).
    Tar(PathBuf),
    /// Remote HTTP(S) source (requires explicit opt-in).
    Remote {
        /// URL of the remote image.
        url: String,
        /// Expected SHA-256 hash for verification.
        sha256: String,
    },
}

/// Resolves an image source URI into an `ImageSource`.
///
/// # Errors
///
/// Returns an error if the URI scheme is unsupported or the path is invalid.
pub fn resolve_source(uri: &str) -> Result<ImageSource> {
    if let Some(path_str) = uri.strip_prefix("file://") {
        let path = PathBuf::from(path_str);
        if !path.exists() {
            return Err(ContainustError::NotFound {
                kind: "image directory",
                id: path_str.to_string(),
            });
        }
        tracing::info!(path = %path.display(), "resolved file:// source");
        Ok(ImageSource::File(path))
    } else if let Some(path_str) = uri.strip_prefix("tar://") {
        let path = PathBuf::from(path_str);
        if !path.exists() {
            return Err(ContainustError::NotFound {
                kind: "tar archive",
                id: path_str.to_string(),
            });
        }
        tracing::info!(path = %path.display(), "resolved tar:// source");
        Ok(ImageSource::Tar(path))
    } else if uri.starts_with("https://") || uri.starts_with("http://") {
        tracing::info!(url = uri, "resolved remote source");
        Ok(ImageSource::Remote {
            url: uri.to_string(),
            sha256: String::new(),
        })
    } else {
        Err(ContainustError::Config {
            message: format!("unsupported image source URI scheme: {uri}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_file_source_existing_dir_returns_file() {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let uri = format!("file://{}", dir.path().display());
        let source = resolve_source(&uri).expect("resolve failed");
        assert!(matches!(source, ImageSource::File(_)));
    }

    #[test]
    fn resolve_tar_source_existing_file_returns_tar() {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let tar_path = dir.path().join("test.tar");
        std::fs::write(&tar_path, b"fake tar").expect("failed to write");
        let uri = format!("tar://{}", tar_path.display());
        let source = resolve_source(&uri).expect("resolve failed");
        assert!(matches!(source, ImageSource::Tar(_)));
    }

    #[test]
    fn resolve_https_source_returns_remote() {
        let source = resolve_source("https://example.com/image.tar").expect("resolve failed");
        assert!(matches!(source, ImageSource::Remote { .. }));
    }

    #[test]
    fn resolve_http_source_returns_remote() {
        let source = resolve_source("http://example.com/image.tar").expect("resolve failed");
        assert!(matches!(source, ImageSource::Remote { .. }));
    }

    #[test]
    fn resolve_unknown_scheme_returns_error() {
        assert!(resolve_source("ftp://example.com/image").is_err());
    }

    #[test]
    fn resolve_missing_file_path_returns_error() {
        assert!(resolve_source("file:///nonexistent/path").is_err());
    }

    #[test]
    fn resolve_missing_tar_path_returns_error() {
        assert!(resolve_source("tar:///nonexistent/archive.tar").is_err());
    }
}
