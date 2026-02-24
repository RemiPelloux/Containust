//! SHA-256 content verification.

use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use containust_common::error::{ContainustError, Result};
use containust_common::types::Sha256Hash;

/// Computes the SHA-256 hash of a file.
///
/// # Errors
///
/// Returns an error if the file cannot be read.
pub fn hash_file(path: &Path) -> Result<Sha256Hash> {
    let mut file = std::fs::File::open(path).map_err(|e| ContainustError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer).map_err(|e| ContainustError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    let hash_bytes = hasher.finalize();
    let hex = format!("{hash_bytes:x}");
    tracing::debug!(path = %path.display(), hash = %hex, "computed SHA-256");
    Sha256Hash::from_hex(hex)
}

/// Validates that a file matches the expected SHA-256 hash.
///
/// # Errors
///
/// Returns `ContainustError::HashMismatch` if the hashes do not match.
pub fn validate_hash(path: &Path, expected: &Sha256Hash) -> Result<()> {
    let actual = hash_file(path)?;
    if actual.as_hex() != expected.as_hex() {
        return Err(ContainustError::HashMismatch {
            resource: path.display().to_string(),
            expected: expected.as_hex().to_string(),
            actual: actual.as_hex().to_string(),
        });
    }
    tracing::debug!(path = %path.display(), "hash validated");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn hash_file_known_content_returns_correct_digest() {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let file_path = dir.path().join("test.txt");
        let mut f = std::fs::File::create(&file_path).expect("failed to create file");
        f.write_all(b"hello world").expect("failed to write");
        drop(f);
        let hash = hash_file(&file_path).expect("hash_file failed");
        assert_eq!(
            hash.as_hex(),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn validate_hash_mismatch_returns_error() {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, b"hello").expect("failed to write");
        let wrong_hash = Sha256Hash::from_hex(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("invalid hex");
        assert!(validate_hash(&file_path, &wrong_hash).is_err());
    }

    #[test]
    fn validate_hash_matching_returns_ok() {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, b"hello world").expect("failed to write");
        let correct_hash = Sha256Hash::from_hex(
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
        )
        .expect("invalid hex");
        assert!(validate_hash(&file_path, &correct_hash).is_ok());
    }

    #[test]
    fn hash_file_nonexistent_returns_error() {
        let result = hash_file(Path::new("/nonexistent/path/file.txt"));
        assert!(result.is_err());
    }
}
