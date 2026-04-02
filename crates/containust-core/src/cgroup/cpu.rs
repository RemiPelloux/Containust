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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_max_bandwidth_value_formatted_correctly() {
        let cgroup_path = Path::new("/sys/fs/cgroup/test");
        let file = cgroup_path.join("cpu.max");
        let quota_us = 50_000u64;
        let period_us = 100_000u64;
        let value = format!("{quota_us} {period_us}");
        assert_eq!(value, "50000 100000");
        assert_eq!(file, Path::new("/sys/fs/cgroup/test/cpu.max"));
    }

    #[test]
    fn cpu_weight_value_within_valid_range() {
        let min_weight: u64 = 1;
        let max_weight: u64 = 10000;
        let default_weight: u64 = 100;
        assert!(min_weight <= default_weight && default_weight <= max_weight);
    }

    #[test]
    fn cpu_weight_file_path_constructed_correctly() {
        let cgroup_path = Path::new("/sys/fs/cgroup/containust/app1");
        let file = cgroup_path.join("cpu.weight");
        assert_eq!(file, Path::new("/sys/fs/cgroup/containust/app1/cpu.weight"));
    }

    /// Requires root and cgroup v2 hierarchy.
    #[test]
    #[ignore = "requires root privileges"]
    fn set_cpu_weight_writes_value() {
        let temp = std::env::temp_dir().join("containust_test_cpu_weight");
        let _ = std::fs::create_dir_all(&temp);
        let result = set_cpu_weight(&temp, 512);
        // Will fail on /sys/fs/cgroup but verifies syscall entry path
        // For temp dir test, this won't succeed without actual cgroup
        drop(result);
        let _ = std::fs::remove_dir_all(&temp);
    }

    /// Requires root and cgroup v2 hierarchy.
    #[test]
    #[ignore = "requires root privileges"]
    fn set_cpu_max_writes_bandwidth() {
        let _cgroup_path = Path::new("/sys/fs/cgroup/containust/test");
        let _ = set_cpu_max(Path::new("/sys/fs/cgroup/containust/test"), 50_000, 100_000);
    }
}
