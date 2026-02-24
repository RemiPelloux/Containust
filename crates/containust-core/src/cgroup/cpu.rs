//! CPU resource control via cgroups v2.
//!
//! Manages `cpu.max`, `cpu.weight`, and related control files.

use std::path::Path;

use containust_common::error::{ContainustError, Result};

/// Sets the CPU weight (shares) for a cgroup.
///
/// Weight is a value between 1 and 10000 that controls the relative
/// share of CPU time this cgroup receives under contention.
///
/// # Errors
///
/// Returns an error if writing to `cpu.weight` fails.
#[cfg(target_os = "linux")]
pub fn set_cpu_weight(cgroup_path: &Path, weight: u64) -> Result<()> {
    let file = cgroup_path.join("cpu.weight");
    std::fs::write(&file, weight.to_string()).map_err(|e| ContainustError::Io {
        path: file,
        source: e,
    })?;
    tracing::debug!(weight, "CPU weight set");
    Ok(())
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — cgroup CPU control requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn set_cpu_weight(_cgroup_path: &Path, _weight: u64) -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}

/// Sets the CPU bandwidth limit (max microseconds per period).
///
/// Writes `quota_us period_us` to `cpu.max`, where `quota_us` is the
/// maximum CPU time allowed per `period_us` window.
///
/// # Errors
///
/// Returns an error if writing to `cpu.max` fails.
#[cfg(target_os = "linux")]
pub fn set_cpu_max(cgroup_path: &Path, quota_us: u64, period_us: u64) -> Result<()> {
    let file = cgroup_path.join("cpu.max");
    let value = format!("{quota_us} {period_us}");
    std::fs::write(&file, value).map_err(|e| ContainustError::Io {
        path: file,
        source: e,
    })?;
    tracing::debug!(quota_us, period_us, "CPU max quota set");
    Ok(())
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — cgroup CPU control requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn set_cpu_max(_cgroup_path: &Path, _quota_us: u64, _period_us: u64) -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}
