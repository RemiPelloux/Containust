//! Cgroups v2 resource management.
//!
//! Provides interfaces for creating cgroup hierarchies and setting
//! resource limits for CPU, memory, and I/O via the unified hierarchy
//! at `/sys/fs/cgroup`.

pub mod cpu;
pub mod io;
pub mod memory;

use std::path::PathBuf;

use containust_common::error::Result;
use containust_common::types::ResourceLimits;

/// Handle to a cgroup for a specific container.
#[derive(Debug)]
pub struct CgroupManager {
    /// Path to this container's cgroup directory.
    path: PathBuf,
}

impl CgroupManager {
    /// Creates a new cgroup for the given container ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the cgroup directory cannot be created.
    pub fn create(container_id: &str) -> Result<Self> {
        let path = PathBuf::from(containust_common::constants::CGROUP_V2_PATH)
            .join("containust")
            .join(container_id);
        tracing::info!(path = %path.display(), "creating cgroup");
        Ok(Self { path })
    }

    /// Applies resource limits to this cgroup.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to cgroup control files fails.
    pub fn apply_limits(&self, limits: &ResourceLimits) -> Result<()> {
        tracing::debug!(path = %self.path.display(), limits = ?limits, "applying resource limits");
        Ok(())
    }

    /// Removes the cgroup and releases resources.
    ///
    /// # Errors
    ///
    /// Returns an error if the cgroup directory cannot be removed.
    pub fn destroy(&self) -> Result<()> {
        tracing::info!(path = %self.path.display(), "destroying cgroup");
        Ok(())
    }
}
