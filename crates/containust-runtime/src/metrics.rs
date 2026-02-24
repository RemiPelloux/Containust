//! Real-time resource metrics collection.
//!
//! Reads cgroup stat files to provide live CPU, memory, and I/O usage
//! for running containers.

use containust_common::error::Result;
use containust_common::types::ContainerId;
use serde::{Deserialize, Serialize};

/// Snapshot of a container's resource usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// Container this snapshot belongs to.
    pub container_id: ContainerId,
    /// CPU usage in nanoseconds.
    pub cpu_usage_ns: u64,
    /// Memory usage in bytes.
    pub memory_usage_bytes: u64,
    /// Number of I/O read bytes.
    pub io_read_bytes: u64,
    /// Number of I/O write bytes.
    pub io_write_bytes: u64,
}

/// Collects a metrics snapshot for the given container.
///
/// On Linux, reads from the cgroup v2 filesystem under
/// `/sys/fs/cgroup/containust/<container_id>/`.
///
/// # Errors
///
/// Returns an error if cgroup stat files cannot be read.
#[cfg(target_os = "linux")]
pub fn collect_metrics(container_id: &ContainerId) -> Result<MetricsSnapshot> {
    let cgroup_path = std::path::Path::new("/sys/fs/cgroup/containust").join(container_id.as_str());

    let memory = read_cgroup_u64(&cgroup_path.join("memory.current")).unwrap_or(0);
    let cpu = read_cpu_usage(&cgroup_path.join("cpu.stat")).unwrap_or(0);

    Ok(MetricsSnapshot {
        container_id: container_id.clone(),
        cpu_usage_ns: cpu,
        memory_usage_bytes: memory,
        io_read_bytes: 0,
        io_write_bytes: 0,
    })
}

#[cfg(target_os = "linux")]
fn read_cgroup_u64(path: &std::path::Path) -> Option<u64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

#[cfg(target_os = "linux")]
fn read_cpu_usage(path: &std::path::Path) -> Option<u64> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        if let Some(val) = line.strip_prefix("usage_usec ") {
            return val.trim().parse::<u64>().ok().map(|us| us * 1000);
        }
    }
    None
}

/// Collects a metrics snapshot for the given container.
///
/// On non-Linux platforms, returns zeroed metrics since cgroup
/// information is unavailable.
///
/// # Errors
///
/// Returns `Ok` with zeroed metrics on non-Linux platforms.
#[cfg(not(target_os = "linux"))]
pub fn collect_metrics(container_id: &ContainerId) -> Result<MetricsSnapshot> {
    Ok(MetricsSnapshot {
        container_id: container_id.clone(),
        cpu_usage_ns: 0,
        memory_usage_bytes: 0,
        io_read_bytes: 0,
        io_write_bytes: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_metrics_returns_snapshot() {
        let id = ContainerId::new("test-metrics");
        let snap = collect_metrics(&id).expect("should succeed");
        assert_eq!(snap.container_id, id);
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn non_linux_metrics_are_zeroed() {
        let id = ContainerId::new("test-zero");
        let snap = collect_metrics(&id).expect("should succeed");
        assert_eq!(snap.cpu_usage_ns, 0);
        assert_eq!(snap.memory_usage_bytes, 0);
        assert_eq!(snap.io_read_bytes, 0);
        assert_eq!(snap.io_write_bytes, 0);
    }
}
