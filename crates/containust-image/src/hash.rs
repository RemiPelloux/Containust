//! SHA-256 content verification and single-pass hashing I/O.

use std::io::{Read, Write};
use std::path::Path;

use sha2::{Digest, Sha256};

use containust_common::error::{ContainustError, Result};
use containust_common::types::Sha256Hash;

/// Chunk size for streaming reads; large enough to keep syscall
/// overhead negligible on multi-hundred-MiB archives.
const IO_BUFFER_BYTES: usize = 128 * 1024;

/// A writer that computes the SHA-256 digest of everything written
/// through it, so callers hash content in the same pass that produces
/// the file instead of re-reading it afterwards.
#[derive(Debug)]
pub struct HashingWriter<W: Write> {
    inner: W,
    hasher: Sha256,
}

impl<W: Write> HashingWriter<W> {
    /// Wraps `inner` so all written bytes are hashed as they pass through.
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
        }
    }

    /// Consumes the writer, returning the inner writer and the digest
    /// of every byte written.
    ///
    /// # Errors
    ///
    /// Returns an error if the digest cannot be encoded (never expected
    /// for a well-formed SHA-256 output).
    pub fn finish(self) -> Result<(W, Sha256Hash)> {
        let digest = Sha256Hash::from_hex(format!("{:x}", self.hasher.finalize()))?;
        Ok((self.inner, digest))
    }
}

impl<W: Write> Write for HashingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let written = self.inner.write(buf)?;
        self.hasher.update(&buf[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

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
    let mut buffer = vec![0_u8; IO_BUFFER_BYTES];
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

    #[test]
    fn hashing_writer_digest_matches_hash_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("streamed.bin");
        let file = std::fs::File::create(&path).expect("create");
        let mut writer = HashingWriter::new(file);
        writer.write_all(b"hello world").expect("write");
        let (_, streamed) = writer.finish().expect("finish");
        let reread = hash_file(&path).expect("hash_file");
        assert_eq!(streamed.as_hex(), reread.as_hex());
    }
}
