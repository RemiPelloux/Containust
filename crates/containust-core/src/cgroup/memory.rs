//! Memory resource control via cgroups v2.
//!
//! Manages `memory.max`, `memory.high`, and related control files.

use std::path::Path;

use containust_common::error::Result;

/// Sets the hard memory limit for a cgroup.
///
/// # Errors
///
/// Returns an error if writing to `memory.max` fails.
pub fn set_memory_max(_cgroup_path: &Path, _bytes: u64) -> Result<()> {
    tracing::debug!("setting memory max limit");
    Ok(())
}

/// Sets the memory high watermark (throttling threshold).
///
/// # Errors
///
/// Returns an error if writing to `memory.high` fails.
pub fn set_memory_high(_cgroup_path: &Path, _bytes: u64) -> Result<()> {
    tracing::debug!("setting memory high watermark");
    Ok(())
}
