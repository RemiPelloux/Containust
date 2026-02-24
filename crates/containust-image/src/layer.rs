//! Filesystem layer management.
//!
//! Each image is composed of ordered layers. Layers are content-addressed
//! by their SHA-256 hash and stored in the local layer cache.

use containust_common::error::Result;
use containust_common::types::Sha256Hash;

/// A single filesystem layer in an image.
#[derive(Debug, Clone)]
pub struct Layer {
    /// Content-addressed hash of this layer.
    pub hash: Sha256Hash,
    /// Size of the layer in bytes.
    pub size_bytes: u64,
}

/// Extracts a layer archive to the layer cache directory.
///
/// # Errors
///
/// Returns an error if extraction or hash validation fails.
pub fn extract_layer(_archive_path: &std::path::Path, _target: &std::path::Path) -> Result<Layer> {
    tracing::info!("extracting layer");
    todo!()
}
