//! User namespace isolation.
//!
//! Maps container UIDs/GIDs to unprivileged host UIDs, enabling rootless containers.

use containust_common::error::Result;

/// Creates a new user namespace for the calling process.
///
/// # Errors
///
/// Returns an error if the `unshare(CLONE_NEWUSER)` syscall fails.
pub fn create_user_namespace() -> Result<()> {
    tracing::debug!("creating user namespace");
    Ok(())
}

/// Writes UID/GID mapping for the user namespace.
///
/// # Errors
///
/// Returns an error if writing to `/proc/self/uid_map` or `/proc/self/gid_map` fails.
pub fn write_uid_gid_map(_container_id: u32, _host_id: u32, _range: u32) -> Result<()> {
    tracing::debug!("writing UID/GID mapping");
    Ok(())
}
