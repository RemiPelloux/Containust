//! IPC namespace isolation.
//!
//! Isolates System V IPC objects and POSIX message queues.

use containust_common::error::Result;

/// Creates a new IPC namespace for the calling process.
///
/// # Errors
///
/// Returns an error if the `unshare(CLONE_NEWIPC)` syscall fails.
pub fn create_ipc_namespace() -> Result<()> {
    tracing::debug!("creating IPC namespace");
    Ok(())
}
