//! PID namespace isolation.
//!
//! Provides the container with its own process ID space, where PID 1
//! is the container's init process.

use containust_common::error::{ContainustError, Result};

/// Creates a new PID namespace for the calling process.
///
/// After a successful call, the next `fork(2)` child will see
/// itself as PID 1 inside the new namespace.
///
/// # Errors
///
/// Returns an error if the `unshare(CLONE_NEWPID)` syscall fails.
#[cfg(target_os = "linux")]
pub fn create_pid_namespace() -> Result<()> {
    use nix::sched::{CloneFlags, unshare};

    unshare(CloneFlags::CLONE_NEWPID).map_err(|e| ContainustError::PermissionDenied {
        message: format!("PID namespace creation failed: {e}"),
    })?;
    tracing::debug!("PID namespace created");
    Ok(())
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — PID namespace requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn create_pid_namespace() -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}

/// Joins an existing PID namespace via its file descriptor.
///
/// # Errors
///
/// Returns an error if `setns(2)` fails.
#[cfg(target_os = "linux")]
pub fn join_pid_namespace(ns_fd: i32) -> Result<()> {
    use nix::sched::{CloneFlags, setns};
    use std::os::fd::BorrowedFd;

    // SAFETY: ns_fd is a valid open file descriptor to a /proc/[pid]/ns/pid file,
    // guaranteed by the caller.
    let fd = unsafe { BorrowedFd::borrow_raw(ns_fd) };
    setns(fd, CloneFlags::CLONE_NEWPID).map_err(|e| ContainustError::PermissionDenied {
        message: format!("setns PID failed: {e}"),
    })?;
    tracing::debug!(ns_fd, "joined PID namespace");
    Ok(())
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — namespace joining requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn join_pid_namespace(_ns_fd: i32) -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}
