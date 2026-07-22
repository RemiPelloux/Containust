//! Real-time resource metrics collection.
//!
//! Reads cgroup stat files to provide live CPU, memory, and I/O usage
//! for running containers. Unavailable fields are reported explicitly
//! rather than silently pretending to be zero when collection failed.

use containust_common::error::Result;
use containust_common::types::ContainerId;
use serde::{Deserialize, Serialize};

/// Whether a metrics field was collected from a live source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricAvailability {
    /// Value was read from the host (cgroup).
    Available,
    /// Platform or backend cannot provide this metric.
    Unavailable,
    /// Collection was attempted but the source was missing.
    Missing,
}

/// Snapshot of a container's resource usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// Container this snapshot belongs to.
    pub container_id: ContainerId,
    /// CPU usage in nanoseconds (cgroup `usage_usec` × 1000 when available).
    pub cpu_usage_ns: u64,
    /// Memory usage in bytes (`memory.current` when available).
    pub memory_usage_bytes: u64,
    /// Number of I/O read bytes (`io.stat` rbytes sum when available).
    pub io_read_bytes: u64,
    /// Number of I/O write bytes (`io.stat` wbytes sum when available).
    pub io_write_bytes: u64,
    /// Availability of CPU metrics.
    pub cpu: MetricAvailability,
    /// Availability of memory metrics.
    pub memory: MetricAvailability,
    /// Availability of I/O metrics.
    pub io: MetricAvailability,
    /// Human-readable note when metrics are degraded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl MetricsSnapshot {
    /// Returns true when at least one live metric field is available.
    #[must_use]
    pub const fn has_live_data(&self) -> bool {
        matches!(self.cpu, MetricAvailability::Available)
            || matches!(self.memory, MetricAvailability::Available)
            || matches!(self.io, MetricAvailability::Available)
    }
}

/// Collects a metrics snapshot for the given container.
///
/// On Linux, reads from the cgroup v2 filesystem under
/// `/sys/fs/cgroup/containust/<container_id>/`.
///
/// # Errors
///
/// Returns an error only for unexpected I/O failures outside normal
/// missing-cgroup cases (those yield `Missing` availability).
#[cfg(target_os = "linux")]
pub fn collect_metrics(container_id: &ContainerId) -> Result<MetricsSnapshot> {
    let cgroup_path = std::path::Path::new("/sys/fs/cgroup/containust").join(container_id.as_str());
    if !cgroup_path.exists() {
        return Ok(unavailable_snapshot(
            container_id,
            MetricAvailability::Missing,
            "cgroup path missing; container may be stopped or not using cgroups",
        ));
    }

    let (memory, memory_av) = read_cgroup_u64(&cgroup_path.join("memory.current"))
        .map_or((0, MetricAvailability::Missing), |value| {
            (value, MetricAvailability::Available)
        });
    let (cpu, cpu_av) = read_cpu_usage(&cgroup_path.join("cpu.stat"))
        .map_or((0, MetricAvailability::Missing), |value| {
            (value, MetricAvailability::Available)
        });
    let (io_read, io_write, io_av) = read_io_stat(&cgroup_path.join("io.stat"))
        .map_or((0, 0, MetricAvailability::Missing), |(r, w)| {
            (r, w, MetricAvailability::Available)
        });

    Ok(MetricsSnapshot {
        container_id: container_id.clone(),
        cpu_usage_ns: cpu,
        memory_usage_bytes: memory,
        io_read_bytes: io_read,
        io_write_bytes: io_write,
        cpu: cpu_av,
        memory: memory_av,
        io: io_av,
        note: None,
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

#[cfg(target_os = "linux")]
fn read_io_stat(path: &std::path::Path) -> Option<(u64, u64)> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut read = 0_u64;
    let mut write = 0_u64;
    let mut found = false;
    for line in content.lines() {
        for token in line.split_whitespace().skip(1) {
            if let Some(value) = token.strip_prefix("rbytes=") {
                read = read.saturating_add(value.parse().ok()?);
                found = true;
            } else if let Some(value) = token.strip_prefix("wbytes=") {
                write = write.saturating_add(value.parse().ok()?);
                found = true;
            }
        }
    }
    found.then_some((read, write))
}

/// Collects a metrics snapshot for the given container.
///
/// On non-Linux platforms, returns unavailable metrics (zeros mean
/// “not collected”, not “idle”).
///
/// # Errors
///
/// Always returns `Ok` on non-Linux platforms.
#[cfg(not(target_os = "linux"))]
pub fn collect_metrics(container_id: &ContainerId) -> Result<MetricsSnapshot> {
    Ok(unavailable_snapshot(
        container_id,
        MetricAvailability::Unavailable,
        "cgroup metrics require Linux native backend",
    ))
}

fn unavailable_snapshot(
    container_id: &ContainerId,
    availability: MetricAvailability,
    note: &str,
) -> MetricsSnapshot {
    MetricsSnapshot {
        container_id: container_id.clone(),
        cpu_usage_ns: 0,
        memory_usage_bytes: 0,
        io_read_bytes: 0,
        io_write_bytes: 0,
        cpu: availability,
        memory: availability,
        io: availability,
        note: Some(note.to_string()),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn collect_metrics_returns_snapshot() {
        let id = ContainerId::new("test-metrics");
        let snap = collect_metrics(&id).expect("should succeed");
        assert_eq!(snap.container_id, id);
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn non_linux_metrics_are_unavailable_not_idle() {
        let id = ContainerId::new("test-zero");
        let snap = collect_metrics(&id).expect("should succeed");
        assert_eq!(snap.cpu, MetricAvailability::Unavailable);
        assert_eq!(snap.memory, MetricAvailability::Unavailable);
        assert_eq!(snap.io, MetricAvailability::Unavailable);
        assert!(!snap.has_live_data());
        assert!(snap.note.as_deref().is_some_and(|n| n.contains("Linux")));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn missing_cgroup_marks_fields_missing() {
        let id = ContainerId::new("definitely-missing-cgroup-id");
        let snap = collect_metrics(&id).expect("ok");
        assert_eq!(snap.cpu, MetricAvailability::Missing);
        assert!(snap.note.is_some());
    }
}
