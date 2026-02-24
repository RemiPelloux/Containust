//! SHA-256 content verification.
//!
//! Validates integrity of downloaded or loaded images and layers.

use std::path::Path;

use containust_common::error::Result;
use containust_common::types::Sha256Hash;

/// Computes the SHA-256 hash of a file.
///
/// # Errors
///
/// Returns an error if the file cannot be read.
pub fn hash_file(_path: &Path) -> Result<Sha256Hash> {
    tracing::debug!(path = %_path.display(), "computing SHA-256 hash");
    todo!()
}

/// Validates that a file matches the expected SHA-256 hash.
///
/// # Errors
///
/// Returns `ContainustError::HashMismatch` if the hashes do not match.
pub fn validate_hash(_path: &Path, _expected: &Sha256Hash) -> Result<()> {
    tracing::debug!(path = %_path.display(), "validating SHA-256 hash");
    todo!()
}
