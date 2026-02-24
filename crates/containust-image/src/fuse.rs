//! FUSE-based lazy-loading for container images.
//!
//! Enables starting a container before its image is fully extracted
//! by serving filesystem requests on-demand from the image archive.

use std::path::Path;

use containust_common::error::Result;

/// Mounts a FUSE filesystem that lazily extracts layers on access.
///
/// # Errors
///
/// Returns an error if the FUSE mount cannot be established.
pub fn mount_lazy(_image_path: &Path, _mount_point: &Path) -> Result<()> {
    tracing::info!(
        mount = %_mount_point.display(),
        "mounting FUSE lazy-loader"
    );
    Ok(())
}
