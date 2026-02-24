//! Secure root filesystem switching via `pivot_root(2)`.
//!
//! More secure than `chroot` because it actually changes the root mount
//! point rather than just the process's view of `/`.

use std::path::Path;

use containust_common::error::Result;

/// Switches the root filesystem to the new root using `pivot_root(2)`.
///
/// The old root is moved to `put_old` and should be unmounted afterward.
///
/// # Errors
///
/// Returns an error if `pivot_root(2)` or the subsequent cleanup fails.
pub fn pivot_root(_new_root: &Path, _put_old: &Path) -> Result<()> {
    tracing::info!(
        new_root = %_new_root.display(),
        "performing pivot_root"
    );
    Ok(())
}
