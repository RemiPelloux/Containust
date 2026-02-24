//! Secure root filesystem switching via `pivot_root(2)`.
//!
//! More secure than `chroot` because it actually changes the root mount
//! point rather than just the process's view of `/`.

use std::path::Path;

use containust_common::error::{ContainustError, Result};

/// Switches the root filesystem to the new root using `pivot_root(2)`.
///
/// Performs the full pivot sequence:
/// 1. Bind-mount `new_root` onto itself (required by `pivot_root`).
/// 2. Create `put_old` directory inside `new_root`.
/// 3. Call `pivot_root(2)` to swap old and new roots.
/// 4. Change working directory to `/`.
/// 5. Lazily unmount and remove the old root.
///
/// # Errors
///
/// Returns an error if any of the mount, pivot, or cleanup operations fail.
#[cfg(target_os = "linux")]
pub fn pivot_root(new_root: &Path, put_old: &Path) -> Result<()> {
    use nix::mount::{MsFlags, mount};

    mount(
        Some(new_root),
        new_root,
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REC,
        None::<&str>,
    )
    .map_err(|e| ContainustError::PermissionDenied {
        message: format!("bind mount for pivot_root failed: {e}"),
    })?;

    std::fs::create_dir_all(put_old).map_err(|e| ContainustError::Io {
        path: put_old.into(),
        source: e,
    })?;

    nix::unistd::pivot_root(new_root, put_old).map_err(|e| ContainustError::PermissionDenied {
        message: format!("pivot_root failed: {e}"),
    })?;

    std::env::set_current_dir("/").map_err(|e| ContainustError::Io {
        path: "/".into(),
        source: e,
    })?;

    nix::mount::umount2("/.old_root", nix::mount::MntFlags::MNT_DETACH).map_err(|e| {
        ContainustError::PermissionDenied {
            message: format!("unmount old root failed: {e}"),
        }
    })?;

    let _ = std::fs::remove_dir("/.old_root");

    tracing::info!("pivot_root complete");
    Ok(())
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — `pivot_root` requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn pivot_root(_new_root: &Path, _put_old: &Path) -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}
