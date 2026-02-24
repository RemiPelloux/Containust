//! I/O resource control via cgroups v2.
//!
//! Manages `io.weight` and `io.max` for block device throttling.

use std::path::Path;

use containust_common::error::{ContainustError, Result};

/// Sets the I/O weight for a cgroup.
///
/// Weight is a value between 1 and 10000 that controls the relative
/// share of I/O bandwidth this cgroup receives under contention.
///
/// # Errors
///
/// Returns an error if writing to `io.weight` fails.
#[cfg(target_os = "linux")]
pub fn set_io_weight(cgroup_path: &Path, weight: u16) -> Result<()> {
    let file = cgroup_path.join("io.weight");
    std::fs::write(&file, weight.to_string()).map_err(|e| ContainustError::Io {
        path: file,
        source: e,
    })?;
    tracing::debug!(weight, "I/O weight set");
    Ok(())
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — cgroup I/O control requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn set_io_weight(_cgroup_path: &Path, _weight: u16) -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}
