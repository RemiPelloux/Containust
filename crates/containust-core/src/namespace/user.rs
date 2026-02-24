//! User namespace isolation.
//!
//! Maps container UIDs/GIDs to unprivileged host UIDs, enabling rootless containers.

use containust_common::error::{ContainustError, Result};

/// Creates a new user namespace for the calling process.
///
/// The calling process gains full privileges within the new namespace,
/// regardless of its privileges in the parent namespace.
///
/// # Errors
///
/// Returns an error if the `unshare(CLONE_NEWUSER)` syscall fails.
#[cfg(target_os = "linux")]
pub fn create_user_namespace() -> Result<()> {
    use nix::sched::{CloneFlags, unshare};

    unshare(CloneFlags::CLONE_NEWUSER).map_err(|e| ContainustError::PermissionDenied {
        message: format!("user namespace creation failed: {e}"),
    })?;
    tracing::debug!("user namespace created");
    Ok(())
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — user namespace requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn create_user_namespace() -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}

/// Writes UID/GID mapping for the user namespace.
///
/// Configures how UIDs/GIDs inside the namespace map to UIDs/GIDs
/// on the host. Must deny `setgroups` first for unprivileged user namespaces.
///
/// # Errors
///
/// Returns an error if writing to `/proc/[pid]/uid_map`,
/// `/proc/[pid]/gid_map`, or `/proc/[pid]/setgroups` fails.
#[cfg(target_os = "linux")]
pub fn write_uid_gid_map(pid: u32, container_id: u32, host_id: u32, range: u32) -> Result<()> {
    use std::fs;

    let uid_map = format!("{container_id} {host_id} {range}");
    let pid_str = if pid == 0 {
        "self".to_string()
    } else {
        pid.to_string()
    };

    let setgroups_path = format!("/proc/{pid_str}/setgroups");
    if std::path::Path::new(&setgroups_path).exists() {
        fs::write(&setgroups_path, "deny").map_err(|e| ContainustError::Io {
            path: setgroups_path.into(),
            source: e,
        })?;
    }

    let uid_map_path = format!("/proc/{pid_str}/uid_map");
    fs::write(&uid_map_path, &uid_map).map_err(|e| ContainustError::Io {
        path: uid_map_path.into(),
        source: e,
    })?;

    let gid_map_path = format!("/proc/{pid_str}/gid_map");
    fs::write(&gid_map_path, &uid_map).map_err(|e| ContainustError::Io {
        path: gid_map_path.into(),
        source: e,
    })?;

    tracing::debug!(pid, container_id, host_id, range, "wrote UID/GID map");
    Ok(())
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — UID/GID mapping requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn write_uid_gid_map(_pid: u32, _container_id: u32, _host_id: u32, _range: u32) -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}
