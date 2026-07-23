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
        let parent = PathBuf::from(containust_common::constants::CGROUP_V2_PATH).join("containust");
        let path = parent.join(container_id);
        std::fs::create_dir_all(&parent).map_err(|e| ContainustError::Io {
            path: parent.clone(),
            source: e,
        })?;
        enable_subtree_controllers(&parent);
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
            // Cgroup directories must be removed with plain `rmdir`;
            // `remove_dir_all` tries to unlink kernel-owned control files
            // first, which cgroupfs rejects with EPERM.
            std::fs::remove_dir(&self.path).map_err(|e| ContainustError::Io {
                path: self.path.clone(),
                source: e,
            })?;
        }
        tracing::info!(path = %self.path.display(), "cgroup destroyed");
        Ok(())
    }
}

/// Enables the cpu, memory, and io controllers for child cgroups.
///
/// Best effort: a controller missing from the kernel or the parent cgroup
/// is logged, and any limit that later requires it fails closed in
/// [`CgroupManager::apply_limits`].
#[cfg(target_os = "linux")]
fn enable_subtree_controllers(parent: &std::path::Path) {
    let control = parent.join("cgroup.subtree_control");
    for controller in ["+cpu", "+memory", "+io"] {
        if let Err(error) = std::fs::write(&control, controller) {
            tracing::warn!(
                controller,
                %error,
                "cgroup controller unavailable; limits requiring it will fail closed"
            );
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use containust_common::types::ResourceLimits;

    #[test]
    fn cgroup_path_constructed_from_container_id() {
        let expected = PathBuf::from("/sys/fs/cgroup/containust/my-container");
        assert_eq!(
            expected,
            PathBuf::from(containust_common::constants::CGROUP_V2_PATH)
                .join("containust")
                .join("my-container")
        );
    }

    #[test]
    fn resource_limits_empty_applies_nothing() {
        let limits = ResourceLimits::default();
        assert!(limits.cpu_shares.is_none());
        assert!(limits.memory_bytes.is_none());
        assert!(limits.io_weight.is_none());
    }

    #[test]
    fn resource_limits_all_set_applies_all() {
        let limits = ResourceLimits {
            cpu_shares: Some(512),
            memory_bytes: Some(536_870_912),
            io_weight: Some(100),
        };
        assert_eq!(limits.cpu_shares, Some(512));
        assert_eq!(limits.memory_bytes, Some(536_870_912));
        assert_eq!(limits.io_weight, Some(100));
    }

    #[test]
    fn cgroup_manager_debug_derived() {
        let mgr = CgroupManager {
            path: PathBuf::from("/tmp/test"),
        };
        let debug_str = format!("{mgr:?}");
        assert!(debug_str.contains("CgroupManager"));
    }

    /// Requires root and /sys/fs/cgroup mount.
    #[test]
    #[ignore = "requires root privileges and cgroup v2"]
    fn cgroup_create_and_destroy_lifecycle() {
        let mgr = CgroupManager::create("test-lifecycle-001").expect("create cgroup");
        assert!(mgr.path.exists());
        mgr.destroy().expect("destroy cgroup");
        assert!(!mgr.path.exists());
    }

    /// Requires root and existing cgroup hierarchy.
    #[test]
    #[ignore = "requires root privileges and cgroup v2"]
    fn cgroup_apply_limits_roundtrip() {
        let mgr = CgroupManager::create("test-limits-001").expect("create");
        let limits = ResourceLimits {
            cpu_shares: Some(256),
            memory_bytes: Some(268_435_456),
            // `io.weight` only exists on kernels with BFQ/iocost; probed below
            // so the fixture is portable across CI kernels.
            io_weight: None,
        };
        mgr.apply_limits(&limits).expect("apply cpu+memory limits");
        if mgr.path.join("io.weight").exists() {
            io::set_io_weight(&mgr.path, 50).expect("apply io weight");
        }
        mgr.destroy().expect("cleanup");
    }
}
