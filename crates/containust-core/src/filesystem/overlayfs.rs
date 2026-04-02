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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_config_clone_and_debug_derived() {
        let config = OverlayConfig {
            lower_dirs: vec![PathBuf::from("/lower1"), PathBuf::from("/lower2")],
            upper_dir: PathBuf::from("/upper"),
            work_dir: PathBuf::from("/work"),
            merged_dir: PathBuf::from("/merged"),
        };
        let cloned = config.clone();
        assert_eq!(format!("{config:?}"), format!("{cloned:?}"));
    }

    #[test]
    fn overlay_config_multiple_lower_dirs_formatted() {
        let config = OverlayConfig {
            lower_dirs: vec![PathBuf::from("/layer1"), PathBuf::from("/layer2")],
            upper_dir: PathBuf::from("/upper"),
            work_dir: PathBuf::from("/work"),
            merged_dir: PathBuf::from("/merged"),
        };
        let lowers = config
            .lower_dirs
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(":");
        assert_eq!(lowers, "/layer1:/layer2");
    }

    #[test]
    fn overlay_config_empty_lower_dirs_valid() {
        let config = OverlayConfig {
            lower_dirs: vec![],
            upper_dir: PathBuf::from("/upper"),
            work_dir: PathBuf::from("/work"),
            merged_dir: PathBuf::from("/merged"),
        };
        assert!(config.lower_dirs.is_empty());
    }

    #[test]
    fn overlay_config_single_lower_dir() {
        let config = OverlayConfig {
            lower_dirs: vec![PathBuf::from("/base_layer")],
            upper_dir: PathBuf::from("/upper"),
            work_dir: PathBuf::from("/work"),
            merged_dir: PathBuf::from("/merged"),
        };
        assert_eq!(config.lower_dirs.len(), 1);
    }

    #[test]
    fn overlay_mount_options_formatted_correctly() {
        let config = OverlayConfig {
            lower_dirs: vec![PathBuf::from("/lower1")],
            upper_dir: PathBuf::from("/upper"),
            work_dir: PathBuf::from("/work"),
            merged_dir: PathBuf::from("/merged"),
        };
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
            config.work_dir.display(),
        );
        assert_eq!(opts, "lowerdir=/lower1,upperdir=/upper,workdir=/work");
    }

    /// Requires root privileges.
    #[test]
    #[ignore = "requires root privileges"]
    fn mount_overlay_creates_directories_and_mounts() {
        let temp = std::env::temp_dir().join("containust_overlay_test");
        let config = OverlayConfig {
            lower_dirs: vec![temp.join("lower")],
            upper_dir: temp.join("upper"),
            work_dir: temp.join("work"),
            merged_dir: temp.join("merged"),
        };
        let _ = std::fs::create_dir_all(&temp);
        mount_overlay(&config).ok();
        let _ = std::fs::remove_dir_all(&temp);
    }

    /// Requires root privileges.
    #[test]
    #[ignore = "requires root privileges"]
    fn unmount_overlay_removes_mount() {
        unmount_overlay(Path::new("/tmp/containust_test_merged")).ok();
    }
}
