//! IPC namespace isolation.
//!
//! Isolates System V IPC objects and POSIX message queues.

use containust_common::error::{ContainustError, Result};

/// Creates a new IPC namespace for the calling process.
///
/// System V IPC objects and POSIX message queues created after this
/// call are invisible to processes in other IPC namespaces.
///
/// # Errors
///
/// Returns an error if the `unshare(CLONE_NEWIPC)` syscall fails.
#[cfg(target_os = "linux")]
pub fn create_ipc_namespace() -> Result<()> {
    use nix::sched::{CloneFlags, unshare};

    unshare(CloneFlags::CLONE_NEWIPC).map_err(|e| ContainustError::PermissionDenied {
        message: format!("IPC namespace creation failed: {e}"),
    })?;
    tracing::debug!("IPC namespace created");
    Ok(())
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — IPC namespace requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn create_ipc_namespace() -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}
