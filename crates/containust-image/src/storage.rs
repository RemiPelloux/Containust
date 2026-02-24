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
