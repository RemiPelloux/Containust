//! Local image catalog management.
//!
//! Maintains an index of available images and their layer compositions.

use containust_common::error::Result;
use containust_common::types::ImageId;

/// Entry in the local image catalog.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImageEntry {
    /// Unique identifier for this image.
    pub id: ImageId,
    /// Human-readable name/tag.
    pub name: String,
    /// Ordered list of layer hashes (bottom to top).
    pub layers: Vec<String>,
}

/// Lists all images in the local catalog.
///
/// # Errors
///
/// Returns an error if the catalog file cannot be read.
pub fn list_images() -> Result<Vec<ImageEntry>> {
    tracing::debug!("listing local images");
    Ok(Vec::new())
}
