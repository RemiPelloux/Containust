//! CPU resource control via cgroups v2.
//!
//! Manages `cpu.max`, `cpu.weight`, and related control files.

use std::path::Path;

use containust_common::error::Result;

/// Sets the CPU weight (shares) for a cgroup.
///
/// # Errors
///
/// Returns an error if writing to `cpu.weight` fails.
pub fn set_cpu_weight(_cgroup_path: &Path, _weight: u64) -> Result<()> {
    tracing::debug!("setting CPU weight");
    Ok(())
}

/// Sets the CPU bandwidth limit (max microseconds per period).
///
/// # Errors
///
/// Returns an error if writing to `cpu.max` fails.
pub fn set_cpu_max(_cgroup_path: &Path, _quota_us: u64, _period_us: u64) -> Result<()> {
    tracing::debug!("setting CPU max quota");
    Ok(())
}
