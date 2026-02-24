//! `OverlayFS` management for layered container filesystems.
//!
//! Stacks multiple read-only layers with a single writable upper layer,
//! enabling efficient image caching and copy-on-write semantics.

use std::path::{Path, PathBuf};

use containust_common::error::{ContainustError, Result};

/// Configuration for an `OverlayFS` mount.
#[derive(Debug, Clone)]
pub struct OverlayConfig {
    /// Read-only lower layers (bottom to top).
    pub lower_dirs: Vec<PathBuf>,
    /// Writable upper layer directory.
    pub upper_dir: PathBuf,
    /// Work directory required by `OverlayFS`.
    pub work_dir: PathBuf,
    /// Final merged mount point.
    pub merged_dir: PathBuf,
}

/// Mounts an `OverlayFS` with the given configuration.
///
/// Creates the upper, work, and merged directories if they do not exist,
/// then issues the `mount(2)` syscall with overlay-specific options.
///
/// # Errors
///
/// Returns an error if directory creation fails or if the mount syscall fails.
#[cfg(target_os = "linux")]
pub fn mount_overlay(config: &OverlayConfig) -> Result<()> {
    use nix::mount::{MsFlags, mount};

    std::fs::create_dir_all(&config.upper_dir).map_err(|e| ContainustError::Io {
        path: config.upper_dir.clone(),
        source: e,
    })?;
    std::fs::create_dir_all(&config.work_dir).map_err(|e| ContainustError::Io {
        path: config.work_dir.clone(),
        source: e,
    })?;
    std::fs::create_dir_all(&config.merged_dir).map_err(|e| ContainustError::Io {
        path: config.merged_dir.clone(),
        source: e,
    })?;

    let lowers = config
        .lower_dirs
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(":");
    let opts = format!(
        "lowerdir={},upperdir={},workdir={}",
        lowers,
        config.upper_dir.display(),
        config.work_dir.display()
    );

    mount(
        Some("overlay"),
        &config.merged_dir,
        Some("overlay"),
        MsFlags::empty(),
        Some(opts.as_str()),
    )
    .map_err(|e| ContainustError::PermissionDenied {
        message: format!("overlay mount failed: {e}"),
    })?;

    tracing::info!(merged = %config.merged_dir.display(), "overlayfs mounted");
    Ok(())
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — `OverlayFS` mounting requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn mount_overlay(_config: &OverlayConfig) -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}

/// Unmounts an `OverlayFS` at the given path.
///
/// Uses `MNT_DETACH` to lazily detach the filesystem.
///
/// # Errors
///
/// Returns an error if the unmount syscall fails.
#[cfg(target_os = "linux")]
pub fn unmount_overlay(merged_dir: &Path) -> Result<()> {
    nix::mount::umount2(merged_dir, nix::mount::MntFlags::MNT_DETACH).map_err(|e| {
        ContainustError::PermissionDenied {
            message: format!("unmount overlay failed: {e}"),
        }
    })?;
    tracing::info!(path = %merged_dir.display(), "overlayfs unmounted");
    Ok(())
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — `OverlayFS` unmounting requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn unmount_overlay(_merged_dir: &Path) -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}
