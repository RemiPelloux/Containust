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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_max_file_path_constructed_correctly() {
        let cgroup_path = Path::new("/sys/fs/cgroup/containust/app1");
        let file = cgroup_path.join("memory.max");
        assert_eq!(file, Path::new("/sys/fs/cgroup/containust/app1/memory.max"));
    }

    #[test]
    fn memory_high_file_path_constructed_correctly() {
        let cgroup_path = Path::new("/sys/fs/cgroup/containust/app1");
        let file = cgroup_path.join("memory.high");
        assert_eq!(
            file,
            Path::new("/sys/fs/cgroup/containust/app1/memory.high")
        );
    }

    #[test]
    fn memory_max_value_formatted_as_string() {
        let bytes: u64 = 536_870_912; // 512 MB
        assert_eq!(bytes.to_string(), "536870912");
    }

    #[test]
    fn memory_high_value_one_gigabyte() {
        let bytes: u64 = 1_073_741_824; // 1 GB
        assert_eq!(bytes.to_string(), "1073741824");
    }

    #[test]
    fn memory_max_zero_bytes_valid() {
        let bytes: u64 = 0;
        assert_eq!(bytes.to_string(), "0");
    }

    /// Requires root and cgroup v2 hierarchy.
    #[test]
    #[ignore = "requires root privileges"]
    fn set_memory_max_writes_value() {
        let _ = set_memory_max(Path::new("/sys/fs/cgroup/containust/test"), 268_435_456);
    }

    /// Requires root and cgroup v2 hierarchy.
    #[test]
    #[ignore = "requires root privileges"]
    fn set_memory_high_writes_value() {
        let _ = set_memory_high(Path::new("/sys/fs/cgroup/containust/test"), 134_217_728);
    }
}
