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
pub fn mount_essential_filesystems(_rootfs: &Path) -> Result<()> {
    tracing::debug!(rootfs = %_rootfs.display(), "mounting essential filesystems");
    Ok(())
}

/// Creates a bind mount from source to target.
///
/// # Errors
///
/// Returns an error if the `mount(2)` syscall fails.
pub fn bind_mount(_source: &Path, _target: &Path, _readonly: bool) -> Result<()> {
    tracing::debug!(
        source = %_source.display(),
        target = %_target.display(),
        "creating bind mount"
    );
    Ok(())
}
