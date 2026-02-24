//! Image source protocol handlers.
//!
//! Supports `file://` (local directory), `tar://` (archive), and
//! remote sources with SHA-256 validation. Local-first by design.

use std::path::PathBuf;

use containust_common::error::Result;

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
pub fn resolve_source(_uri: &str) -> Result<ImageSource> {
    tracing::debug!(uri = _uri, "resolving image source");
    todo!()
}
