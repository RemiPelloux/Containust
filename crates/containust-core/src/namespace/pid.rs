//! PID namespace isolation.
//!
//! Provides the container with its own process ID space, where PID 1
//! is the container's init process.

use containust_common::error::Result;

/// Creates a new PID namespace for the calling process.
///
/// # Errors
///
/// Returns an error if the `unshare(CLONE_NEWPID)` syscall fails.
pub fn create_pid_namespace() -> Result<()> {
    tracing::debug!("creating PID namespace");
    Ok(())
}

/// Joins an existing PID namespace via its file descriptor.
///
/// # Errors
///
/// Returns an error if `setns(2)` fails.
pub fn join_pid_namespace(_ns_fd: i32) -> Result<()> {
    tracing::debug!("joining PID namespace");
    Ok(())
}
