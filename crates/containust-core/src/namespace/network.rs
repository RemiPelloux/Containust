//! Network namespace isolation.
//!
//! Provides the container with its own network stack (interfaces, routing, iptables).

use containust_common::error::{ContainustError, Result};

/// Creates a new network namespace for the calling process.
///
/// The new namespace starts with only a loopback interface.
///
/// # Errors
///
/// Returns an error if the `unshare(CLONE_NEWNET)` syscall fails.
#[cfg(target_os = "linux")]
pub fn create_network_namespace() -> Result<()> {
    use nix::sched::{CloneFlags, unshare};

    unshare(CloneFlags::CLONE_NEWNET).map_err(|e| ContainustError::PermissionDenied {
        message: format!("network namespace creation failed: {e}"),
    })?;
    tracing::debug!("network namespace created");
    Ok(())
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — network namespace requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn create_network_namespace() -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}
