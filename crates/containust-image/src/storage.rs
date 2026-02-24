//! Local storage backend for images and layers.
//!
//! Manages the on-disk layout of layer caches and image metadata
//! under the configured data directory.

use std::path::{Path, PathBuf};

use containust_common::error::Result;

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
    /// Returns an error if the directory cannot be created or accessed.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        tracing::info!(path = %root.display(), "opening storage backend");
        Ok(Self { root })
    }

    /// Returns the path to a layer's directory given its hash.
    #[must_use]
    pub fn layer_path(&self, hash: &str) -> PathBuf {
        self.root.join("layers").join(hash)
    }

    /// Checks whether a layer exists in the local cache.
    #[must_use]
    pub fn has_layer(&self, hash: &str) -> bool {
        self.layer_path(hash).exists()
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
    fn storage_has_layer_true_when_present() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = StorageBackend::open(dir.path().to_path_buf()).expect("open");
        let layer_dir = storage.layer_path("exists");
        std::fs::create_dir_all(&layer_dir).expect("mkdir");
        assert!(storage.has_layer("exists"));
    }
}
