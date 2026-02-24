//! Cgroups v2 resource management.
//!
//! Provides interfaces for creating cgroup hierarchies and setting
//! resource limits for CPU, memory, and I/O via the unified hierarchy
//! at `/sys/fs/cgroup`.

pub mod cpu;
pub mod io;
pub mod memory;

use std::path::PathBuf;

use containust_common::error::{ContainustError, Result};
use containust_common::types::ResourceLimits;

/// Handle to a cgroup for a specific container.
#[derive(Debug)]
pub struct CgroupManager {
    /// Path to this container's cgroup directory.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    path: PathBuf,
}

#[cfg(target_os = "linux")]
impl CgroupManager {
    /// Creates a new cgroup for the given container ID.
    ///
    /// The cgroup is placed under `/sys/fs/cgroup/containust/<container_id>`.
    ///
    /// # Errors
    ///
    /// Returns an error if the cgroup directory cannot be created.
    pub fn create(container_id: &str) -> Result<Self> {
        let path = PathBuf::from(containust_common::constants::CGROUP_V2_PATH)
            .join("containust")
            .join(container_id);
        std::fs::create_dir_all(&path).map_err(|e| ContainustError::Io {
            path: path.clone(),
            source: e,
        })?;
        tracing::info!(path = %path.display(), "cgroup created");
        Ok(Self { path })
    }

    /// Applies resource limits to this cgroup.
    ///
    /// Delegates to subsystem-specific writers for CPU, memory, and I/O.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to cgroup control files fails.
    pub fn apply_limits(&self, limits: &ResourceLimits) -> Result<()> {
        if let Some(mem) = limits.memory_bytes {
            memory::set_memory_max(&self.path, mem)?;
        }
        if let Some(cpu_weight) = limits.cpu_shares {
            cpu::set_cpu_weight(&self.path, cpu_weight)?;
        }
        if let Some(io_weight) = limits.io_weight {
            io::set_io_weight(&self.path, io_weight)?;
        }
        Ok(())
    }

    /// Adds a process to this cgroup by writing its PID.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to `cgroup.procs` fails.
    pub fn add_process(&self, pid: u32) -> Result<()> {
        let procs_path = self.path.join("cgroup.procs");
        std::fs::write(&procs_path, pid.to_string()).map_err(|e| ContainustError::Io {
            path: procs_path,
            source: e,
        })?;
        tracing::debug!(pid, "added process to cgroup");
        Ok(())
    }

    /// Removes the cgroup and releases resources.
    ///
    /// # Errors
    ///
    /// Returns an error if the cgroup directory cannot be removed.
    pub fn destroy(&self) -> Result<()> {
        if self.path.exists() {
            std::fs::remove_dir_all(&self.path).map_err(|e| ContainustError::Io {
                path: self.path.clone(),
                source: e,
            })?;
        }
        tracing::info!(path = %self.path.display(), "cgroup destroyed");
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
impl CgroupManager {
    /// Stub for non-Linux platforms.
    ///
    /// # Errors
    ///
    /// Always returns an error — cgroup management requires Linux.
    pub fn create(_container_id: &str) -> Result<Self> {
        Err(ContainustError::Config {
            message: "Linux required for native container operations".into(),
        })
    }

    /// Stub for non-Linux platforms.
    ///
    /// # Errors
    ///
    /// Always returns an error — cgroup management requires Linux.
    pub fn apply_limits(&self, _limits: &ResourceLimits) -> Result<()> {
        Err(ContainustError::Config {
            message: "Linux required for native container operations".into(),
        })
    }

    /// Stub for non-Linux platforms.
    ///
    /// # Errors
    ///
    /// Always returns an error — cgroup management requires Linux.
    pub fn add_process(&self, _pid: u32) -> Result<()> {
        Err(ContainustError::Config {
            message: "Linux required for native container operations".into(),
        })
    }

    /// Stub for non-Linux platforms.
    ///
    /// # Errors
    ///
    /// Always returns an error — cgroup management requires Linux.
    pub fn destroy(&self) -> Result<()> {
        Err(ContainustError::Config {
            message: "Linux required for native container operations".into(),
        })
    }
}
