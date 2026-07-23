//! Image source protocol handlers.
//!
//! Resolves image URIs into filesystem-checked [`ImageSource`] values.
//! Supports `file://` (local directory), `tar://` (archive), `image://`
//! (local catalog), and remote sources. Local-first by design; parsing
//! itself is delegated to [`crate::reference::ImageReference`].

use std::path::PathBuf;

use containust_common::error::{ContainustError, Result};

use crate::reference::{ImageReference, ImageScheme};

/// Supported image source protocols.
#[derive(Debug, Clone)]
pub enum ImageSource {
    /// Local directory (`file:///path/to/rootfs`).
    File(PathBuf),
    /// Local tar archive (`tar:///path/to/image.tar`).
    Tar(PathBuf),
    /// Image imported into the local catalog (`image://name`).
    Catalog {
        /// Catalog name of the image.
        name: String,
        /// Pinned SHA-256 digest, if any.
        sha256: Option<String>,
    },
    /// Curated well-known rootfs (`preset://alpine`).
    Preset {
        /// Preset name and optional version (`alpine` or `alpine:3.21`).
        name: String,
    },
    /// Remote HTTP(S) source (requires explicit opt-in).
    Remote {
        /// URL of the remote image.
        url: String,
        /// Expected SHA-256 hash for verification.
        sha256: String,
    },
    /// OCI registry image (`oci://name:tag`), pulled via `ctst pull`.
    Oci {
        /// Registry image name (`[registry/]repository[:tag]`).
        name: String,
        /// Pinned top-level manifest digest, when provided.
        sha256: Option<String>,
    },
}

/// Resolves an image source URI into an `ImageSource`.
///
/// Local `file://` and `tar://` paths are checked for existence.
///
/// # Errors
///
/// Returns an error if the URI scheme is unsupported or a local path
/// does not exist.
pub fn resolve_source(uri: &str) -> Result<ImageSource> {
    let reference = ImageReference::parse(uri)?;
    let digest_hex = reference.digest().map(|digest| digest.as_hex().to_string());
    match reference.scheme() {
        ImageScheme::File => {
            let path = existing_path(reference.location(), "image directory")?;
            tracing::info!(path = %path.display(), "resolved file:// source");
            Ok(ImageSource::File(path))
        }
        ImageScheme::Tar => {
            let path = existing_path(reference.location(), "tar archive")?;
            tracing::info!(path = %path.display(), "resolved tar:// source");
            Ok(ImageSource::Tar(path))
        }
        ImageScheme::Catalog => Ok(ImageSource::Catalog {
            name: reference.location().to_string(),
            sha256: digest_hex,
        }),
        ImageScheme::Preset => Ok(ImageSource::Preset {
            name: reference.location().to_string(),
        }),
        ImageScheme::Https | ImageScheme::Http => Ok(ImageSource::Remote {
            url: reference.canonical_uri(),
            sha256: digest_hex.unwrap_or_default(),
        }),
        ImageScheme::Oci => Ok(ImageSource::Oci {
            name: reference.location().to_string(),
            sha256: digest_hex,
        }),
    }
}

fn existing_path(location: &str, kind: &'static str) -> Result<PathBuf> {
    let path = PathBuf::from(location);
    if !path.exists() {
        return Err(ContainustError::NotFound {
            kind,
            id: location.to_string(),
        });
    }
    Ok(path)
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
    fn resolve_catalog_source_returns_name_and_digest() {
        let digest = "a".repeat(64);
        let source =
            resolve_source(&format!("image://web@sha256:{digest}")).expect("resolve failed");
        let ImageSource::Catalog { name, sha256 } = source else {
            unreachable!("expected Catalog source");
        };
        assert_eq!(name, "web");
        assert_eq!(sha256, Some(digest));
    }

    #[test]
    fn resolve_https_source_returns_remote() {
        let source = resolve_source("https://example.com/image.tar").expect("resolve failed");
        assert!(matches!(source, ImageSource::Remote { .. }));
    }

    #[test]
    fn resolve_https_source_with_digest_populates_sha256() {
        let digest = "b".repeat(64);
        let source = resolve_source(&format!("https://example.com/image.tar@sha256:{digest}"))
            .expect("resolve failed");
        let ImageSource::Remote { sha256, .. } = source else {
            unreachable!("expected Remote source");
        };
        assert_eq!(sha256, digest);
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
