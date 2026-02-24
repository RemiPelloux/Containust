//! Mount namespace isolation.
//!
//! Gives the container its own mount table, enabling private filesystem views.

use containust_common::error::{ContainustError, Result};

/// Creates a new mount namespace for the calling process.
///
/// After this call, mount/unmount operations are invisible to other processes.
///
/// # Errors
///
/// Returns an error if the `unshare(CLONE_NEWNS)` syscall fails.
#[cfg(target_os = "linux")]
pub fn create_mount_namespace() -> Result<()> {
    use nix::sched::{CloneFlags, unshare};

    unshare(CloneFlags::CLONE_NEWNS).map_err(|e| ContainustError::PermissionDenied {
        message: format!("mount namespace creation failed: {e}"),
    })?;
    tracing::debug!("mount namespace created");
    Ok(())
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — mount namespace requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn create_mount_namespace() -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}
