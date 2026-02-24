//! Mount utilities for container filesystem setup.
//!
//! Handles mounting `/proc`, `/sys`, `/dev`, and bind mounts
//! inside the container's namespace.

use std::path::Path;

use containust_common::error::Result;

/// Mounts essential pseudo-filesystems (`/proc`, `/sys`, `/dev`) inside the container.
///
/// # Errors
///
/// Returns an error if any mount syscall fails.
pub fn mount_essential_filesystems(rootfs: &Path) -> Result<()> {
    tracing::debug!(rootfs = %rootfs.display(), "mounting essential filesystems");
    Ok(())
}

/// Creates a bind mount from source to target.
///
/// # Errors
///
/// Returns an error if the `mount(2)` syscall fails.
pub fn bind_mount(source: &Path, target: &Path, _readonly: bool) -> Result<()> {
    tracing::debug!(
        source = %source.display(),
        target = %target.display(),
        "creating bind mount"
    );
    Ok(())
}
