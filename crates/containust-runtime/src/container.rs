//! Core container struct and lifecycle operations.

use containust_common::error::{ContainustError, Result};
use containust_common::types::{ContainerId, ContainerState, ResourceLimits};

/// A container instance with its configuration and runtime state.
#[derive(Debug)]
pub struct Container {
    /// Unique identifier.
    pub id: ContainerId,
    /// Human-readable name.
    pub name: String,
    /// Current lifecycle state.
    pub state: ContainerState,
    /// Resource limits applied to the cgroup.
    pub limits: ResourceLimits,
    /// Command to execute inside the container.
    pub command: Vec<String>,
    /// Environment variables passed to the process.
    pub env: Vec<(String, String)>,
    /// PID of the container's init process (if running).
    pub pid: Option<u32>,
    /// Image source URI.
    pub image_source: String,
    /// Path to the container's root filesystem.
    pub rootfs_path: Option<std::path::PathBuf>,
    /// Path to the container's log file.
    pub log_path: Option<std::path::PathBuf>,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
}

impl Container {
    /// Creates a new container in the `Created` state.
    #[must_use]
    pub fn new(id: ContainerId, name: String, command: Vec<String>) -> Self {
        Self {
            id,
            name,
            state: ContainerState::Created,
            limits: ResourceLimits::default(),
            command,
            env: Vec::new(),
            pid: None,
            image_source: String::new(),
            rootfs_path: None,
            log_path: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Starts the container, transitioning to `Running`.
    ///
    /// Spawns a process inside the given rootfs using chroot isolation.
    ///
    /// # Errors
    ///
    /// Returns an error if the container is already running or process
    /// spawning fails.
    pub fn start(&mut self, rootfs: &std::path::Path) -> Result<()> {
        if self.state == ContainerState::Running {
            return Err(ContainustError::Config {
                message: format!("container {} is already running", self.id),
            });
        }

        let pid = crate::process::spawn_container_process(&self.command, &self.env, rootfs)?;
        self.pid = Some(pid);
        self.rootfs_path = Some(rootfs.to_path_buf());
        self.state = ContainerState::Running;
        tracing::info!(id = %self.id, pid, "container started");
        Ok(())
    }

    /// Stops the container, transitioning to `Stopped`.
    ///
    /// On Linux, sends SIGTERM followed by SIGKILL if the process
    /// does not exit within the grace period.
    ///
    /// # Errors
    ///
    /// Returns an error if the process cannot be signaled.
    #[cfg(target_os = "linux")]
    pub fn stop(&mut self) -> Result<()> {
        if let Some(pid) = self.pid {
            use nix::sys::signal::{Signal, kill};
            use nix::unistd::Pid;

            let nix_pid = Pid::from_raw(pid as i32);

            if kill(nix_pid, Signal::SIGTERM).is_ok() {
                tracing::info!(pid, "sent SIGTERM");
                std::thread::sleep(std::time::Duration::from_secs(2));

                if kill(nix_pid, None).is_ok() {
                    let _ = kill(nix_pid, Signal::SIGKILL);
                    tracing::info!(pid, "sent SIGKILL");
                }
            }
        }

        self.state = ContainerState::Stopped;
        self.pid = None;
        tracing::info!(id = %self.id, "container stopped");
        Ok(())
    }

    /// Stops the container, transitioning to `Stopped`.
    ///
    /// On non-Linux platforms, simply transitions state without
    /// sending signals.
    ///
    /// # Errors
    ///
    /// Returns `Ok(())` unconditionally on non-Linux platforms.
    #[cfg(not(target_os = "linux"))]
    pub const fn stop(&mut self) -> Result<()> {
        self.state = ContainerState::Stopped;
        self.pid = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_container_has_created_state() {
        let id = ContainerId::new("test-1");
        let c = Container::new(id, "test".into(), vec!["sh".into()]);
        assert_eq!(c.state, ContainerState::Created);
        assert!(c.pid.is_none());
        assert!(c.rootfs_path.is_none());
    }

    #[test]
    fn new_container_stores_name_and_command() {
        let id = ContainerId::new("test-2");
        let c = Container::new(id, "my-app".into(), vec!["./run".into(), "--flag".into()]);
        assert_eq!(c.name, "my-app");
        assert_eq!(c.command, vec!["./run", "--flag"]);
    }

    #[test]
    fn stop_on_created_container_transitions_to_stopped() {
        let id = ContainerId::new("test-3");
        let mut c = Container::new(id, "test".into(), vec!["sh".into()]);
        c.stop().expect("stop should succeed");
        assert_eq!(c.state, ContainerState::Stopped);
    }
}
