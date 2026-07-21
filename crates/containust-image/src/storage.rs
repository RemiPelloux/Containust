//! Local storage backend for images and layers.
//!
//! Manages the on-disk layout of the content-addressed layer store
//! under the configured data directory. Layer blobs are staged in a
//! temporary file and committed with an atomic rename so interrupted
//! writes never produce a partially written, addressable layer.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use containust_common::error::{ContainustError, Result};

static STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);

const LAYER_BLOB_NAME: &str = "layer.tar";

/// Manages local storage of images and layers.
#[derive(Debug)]
pub struct StorageBackend {
    /// Root directory for all stored data.
    root: PathBuf,
}

impl StorageBackend {
    /// Opens or initializes the storage backend at the given root.
    ///
    /// # Errors
    ///
    /// Returns an error if the layer directory cannot be created.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let layers = root.join("layers");
        std::fs::create_dir_all(&layers).map_err(|source| ContainustError::Io {
            path: layers,
            source,
        })?;
        tracing::info!(path = %root.display(), "opened storage backend");
        Ok(Self { root })
    }

    /// Returns the path to a layer's directory given its hash.
    #[must_use]
    pub fn layer_path(&self, hash: &str) -> PathBuf {
        self.root.join("layers").join(hash)
    }

    /// Returns the path of the archived blob for a stored layer.
    #[must_use]
    pub fn layer_blob_path(&self, hash: &str) -> PathBuf {
        self.layer_path(hash).join(LAYER_BLOB_NAME)
    }

    /// Checks whether a layer blob exists in the local cache.
    #[must_use]
    pub fn has_layer(&self, hash: &str) -> bool {
        self.layer_blob_path(hash).exists()
    }

    /// Returns a unique staging path for building a layer blob.
    ///
    /// The caller writes the candidate blob here and then commits it
    /// with [`Self::commit_layer`] once its content hash is known.
    #[must_use]
    pub fn staging_path(&self) -> PathBuf {
        let counter = STAGING_COUNTER.fetch_add(1, Ordering::Relaxed);
        self.root
            .join("layers")
            .join(format!(".staging-{}-{counter}", std::process::id()))
    }

    /// Atomically commits a staged blob as the layer for `hash`.
    ///
    /// Committing an already-present layer discards the staged copy,
    /// keeping the store idempotent for repeated imports.
    ///
    /// # Errors
    ///
    /// Returns an error if the layer directory cannot be created or
    /// the staged file cannot be moved into place.
    pub fn commit_layer(&self, staged: &Path, hash: &str) -> Result<()> {
        if self.has_layer(hash) {
            std::fs::remove_file(staged).map_err(|source| ContainustError::Io {
                path: staged.to_path_buf(),
                source,
            })?;
            return Ok(());
        }
        let layer_dir = self.layer_path(hash);
        std::fs::create_dir_all(&layer_dir).map_err(|source| ContainustError::Io {
            path: layer_dir.clone(),
            source,
        })?;
        let blob = self.layer_blob_path(hash);
        std::fs::rename(staged, &blob)
            .map_err(|source| ContainustError::Io { path: blob, source })?;
        Ok(())
    }

    /// Returns the root storage path.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_open_returns_correct_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = StorageBackend::open(dir.path().to_path_buf()).expect("open");
        assert_eq!(storage.root(), dir.path());
    }

    #[test]
    fn storage_layer_path_includes_hash() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = StorageBackend::open(dir.path().to_path_buf()).expect("open");
        let path = storage.layer_path("abc123");
        assert!(path.ends_with("layers/abc123"));
    }

    #[test]
    fn storage_has_layer_false_when_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = StorageBackend::open(dir.path().to_path_buf()).expect("open");
        assert!(!storage.has_layer("nonexistent"));
    }

    #[test]
    fn storage_commit_layer_makes_blob_addressable() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = StorageBackend::open(dir.path().to_path_buf()).expect("open");
        let staged = storage.staging_path();
        std::fs::write(&staged, b"layer bytes").expect("write staged");

        storage.commit_layer(&staged, "abc123").expect("commit");

        assert!(storage.has_layer("abc123"));
        let content = std::fs::read(storage.layer_blob_path("abc123")).expect("read blob");
        assert_eq!(content, b"layer bytes");
        assert!(!staged.exists());
    }

    #[test]
    fn storage_commit_existing_layer_discards_staged_copy() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = StorageBackend::open(dir.path().to_path_buf()).expect("open");
        let first = storage.staging_path();
        std::fs::write(&first, b"original").expect("write first");
        storage.commit_layer(&first, "dup").expect("commit first");

        let second = storage.staging_path();
        std::fs::write(&second, b"replacement").expect("write second");
        storage.commit_layer(&second, "dup").expect("commit second");

        let content = std::fs::read(storage.layer_blob_path("dup")).expect("read blob");
        assert_eq!(content, b"original");
        assert!(!second.exists());
    }

    #[test]
    fn storage_staging_paths_are_unique() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = StorageBackend::open(dir.path().to_path_buf()).expect("open");
        assert_ne!(storage.staging_path(), storage.staging_path());
    }
}
