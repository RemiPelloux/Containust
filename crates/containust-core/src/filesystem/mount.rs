//! Mount utilities for container filesystem setup.
//!
//! Handles mounting `/proc`, `/sys`, `/dev`, and bind mounts
//! inside the container's namespace.

use std::path::Path;

use containust_common::error::{ContainustError, Result};

/// Mounts essential pseudo-filesystems (`/proc`, `/sys`, `/dev`) inside the container.
///
/// - `/proc` is mounted with `nosuid`, `nodev`, `noexec`.
/// - `/sys` is mounted read-only with `nosuid`, `nodev`, `noexec`.
/// - `/dev` is a `tmpfs` with `nosuid` and `strictatime`.
///
/// # Errors
///
/// Returns an error if any mount syscall fails.
#[cfg(target_os = "linux")]
pub fn mount_essential_filesystems(rootfs: &Path) -> Result<()> {
    use nix::mount::{MsFlags, mount};

    let proc_path = rootfs.join("proc");
    std::fs::create_dir_all(&proc_path).map_err(|e| ContainustError::Io {
        path: proc_path.clone(),
        source: e,
    })?;
    mount(
        Some("proc"),
        &proc_path,
        Some("proc"),
        MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
        None::<&str>,
    )
    .map_err(|e| ContainustError::PermissionDenied {
        message: format!("mount /proc failed: {e}"),
    })?;

    let sys_path = rootfs.join("sys");
    std::fs::create_dir_all(&sys_path).map_err(|e| ContainustError::Io {
        path: sys_path.clone(),
        source: e,
    })?;
    mount(
        Some("sysfs"),
        &sys_path,
        Some("sysfs"),
        MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC | MsFlags::MS_RDONLY,
        None::<&str>,
    )
    .map_err(|e| ContainustError::PermissionDenied {
        message: format!("mount /sys failed: {e}"),
    })?;

    let dev_path = rootfs.join("dev");
    std::fs::create_dir_all(&dev_path).map_err(|e| ContainustError::Io {
        path: dev_path.clone(),
        source: e,
    })?;
    mount(
        Some("tmpfs"),
        &dev_path,
        Some("tmpfs"),
        MsFlags::MS_NOSUID | MsFlags::MS_STRICTATIME,
        Some("mode=755,size=65536k"),
    )
    .map_err(|e| ContainustError::PermissionDenied {
        message: format!("mount /dev failed: {e}"),
    })?;

    tracing::debug!(rootfs = %rootfs.display(), "essential filesystems mounted");
    Ok(())
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — filesystem mounting requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn mount_essential_filesystems(_rootfs: &Path) -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}

/// Creates a bind mount from source to target.
///
/// If `readonly` is true, the mount is remounted read-only after binding.
///
/// # Errors
///
/// Returns an error if the `mount(2)` syscall fails.
#[cfg(target_os = "linux")]
pub fn bind_mount(source: &Path, target: &Path, readonly: bool) -> Result<()> {
    use nix::mount::{MsFlags, mount};

    std::fs::create_dir_all(target).map_err(|e| ContainustError::Io {
        path: target.into(),
        source: e,
    })?;

    mount(
        Some(source),
        target,
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REC,
        None::<&str>,
    )
    .map_err(|e| ContainustError::PermissionDenied {
        message: format!("bind mount failed: {e}"),
    })?;

    if readonly {
        mount(
            None::<&str>,
            target,
            None::<&str>,
            MsFlags::MS_REMOUNT | MsFlags::MS_BIND | MsFlags::MS_RDONLY,
            None::<&str>,
        )
        .map_err(|e| ContainustError::PermissionDenied {
            message: format!("readonly remount failed: {e}"),
        })?;
    }

    tracing::debug!(
        source = %source.display(),
        target = %target.display(),
        readonly,
        "bind mount created"
    );
    Ok(())
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — bind mounting requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn bind_mount(_source: &Path, _target: &Path, _readonly: bool) -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}
