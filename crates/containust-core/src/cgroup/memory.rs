//! Memory resource control via cgroups v2.
//!
//! Manages `memory.max`, `memory.high`, and related control files.

use std::path::Path;

use containust_common::error::{ContainustError, Result};

/// Sets the hard memory limit for a cgroup.
///
/// Processes exceeding this limit are subject to the OOM killer.
///
/// # Errors
///
/// Returns an error if writing to `memory.max` fails.
#[cfg(target_os = "linux")]
pub fn set_memory_max(cgroup_path: &Path, bytes: u64) -> Result<()> {
    let file = cgroup_path.join("memory.max");
    std::fs::write(&file, bytes.to_string()).map_err(|e| ContainustError::Io {
        path: file,
        source: e,
    })?;
    tracing::debug!(bytes, "memory max limit set");
    Ok(())
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — cgroup memory control requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn set_memory_max(_cgroup_path: &Path, _bytes: u64) -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}

/// Sets the memory high watermark (throttling threshold).
///
/// Processes exceeding this limit are throttled but not killed.
///
/// # Errors
///
/// Returns an error if writing to `memory.high` fails.
#[cfg(target_os = "linux")]
pub fn set_memory_high(cgroup_path: &Path, bytes: u64) -> Result<()> {
    let file = cgroup_path.join("memory.high");
    std::fs::write(&file, bytes.to_string()).map_err(|e| ContainustError::Io {
        path: file,
        source: e,
    })?;
    tracing::debug!(bytes, "memory high watermark set");
    Ok(())
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — cgroup memory control requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn set_memory_high(_cgroup_path: &Path, _bytes: u64) -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}
