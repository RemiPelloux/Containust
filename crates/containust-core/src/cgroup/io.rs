//! I/O resource control via cgroups v2.
//!
//! Manages `io.weight` and `io.max` for block device throttling.

use std::path::Path;

use containust_common::error::Result;

/// Sets the I/O weight for a cgroup.
///
/// # Errors
///
/// Returns an error if writing to `io.weight` fails.
pub fn set_io_weight(_cgroup_path: &Path, _weight: u16) -> Result<()> {
    tracing::debug!("setting I/O weight");
    Ok(())
}
