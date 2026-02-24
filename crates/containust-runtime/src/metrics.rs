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
/// # Errors
///
/// Returns an error if cgroup stat files cannot be read.
pub fn collect_metrics(container_id: &ContainerId) -> Result<MetricsSnapshot> {
    tracing::debug!(id = %container_id, "collecting metrics");
    todo!()
}
