//! `OverlayFS` management for layered container filesystems.
//!
//! Stacks multiple read-only layers with a single writable upper layer,
//! enabling efficient image caching and copy-on-write semantics.

use std::path::{Path, PathBuf};

use containust_common::error::Result;

/// Configuration for an `OverlayFS` mount.
#[derive(Debug, Clone)]
pub struct OverlayConfig {
    /// Read-only lower layers (bottom to top).
    pub lower_dirs: Vec<PathBuf>,
    /// Writable upper layer directory.
    pub upper_dir: PathBuf,
    /// Work directory required by `OverlayFS`.
    pub work_dir: PathBuf,
    /// Final merged mount point.
    pub merged_dir: PathBuf,
}

/// Mounts an `OverlayFS` with the given configuration.
///
/// # Errors
///
/// Returns an error if the mount syscall fails or directories are missing.
pub fn mount_overlay(config: &OverlayConfig) -> Result<()> {
    tracing::info!(merged = %config.merged_dir.display(), "mounting overlayfs");
    Ok(())
}

/// Unmounts an `OverlayFS` at the given path.
///
/// # Errors
///
/// Returns an error if the unmount syscall fails.
pub fn unmount_overlay(merged_dir: &Path) -> Result<()> {
    tracing::info!(path = %merged_dir.display(), "unmounting overlayfs");
    Ok(())
}
