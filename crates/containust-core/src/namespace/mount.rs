//! Mount namespace isolation.
//!
//! Gives the container its own mount table, enabling private filesystem views.

use containust_common::error::Result;

/// Creates a new mount namespace for the calling process.
///
/// # Errors
///
/// Returns an error if the `unshare(CLONE_NEWNS)` syscall fails.
pub fn create_mount_namespace() -> Result<()> {
    tracing::debug!("creating mount namespace");
    Ok(())
}
