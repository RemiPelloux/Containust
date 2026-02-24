//! Core container struct and lifecycle operations.

use containust_common::error::Result;
use containust_common::types::{ContainerId, ContainerState, ResourceLimits};

/// A container instance with its configuration and runtime state.
#[derive(Debug)]
pub struct Container {
    /// Unique identifier.
    pub id: ContainerId,
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
}

impl Container {
    /// Creates a new container in the `Created` state.
    #[must_use]
    pub fn new(id: ContainerId, command: Vec<String>) -> Self {
        Self {
            id,
            state: ContainerState::Created,
            limits: ResourceLimits::default(),
            command,
            env: Vec::new(),
            pid: None,
        }
    }

    /// Starts the container, transitioning to `Running`.
    ///
    /// # Errors
    ///
    /// Returns an error if namespace creation, cgroup setup, or process spawning fails.
    pub fn start(&mut self) -> Result<()> {
        tracing::info!(id = %self.id, "starting container");
        self.state = ContainerState::Running;
        Ok(())
    }

    /// Stops the container, transitioning to `Stopped`.
    ///
    /// # Errors
    ///
    /// Returns an error if the process cannot be signaled or cgroup cleanup fails.
    pub fn stop(&mut self) -> Result<()> {
        tracing::info!(id = %self.id, "stopping container");
        self.state = ContainerState::Stopped;
        self.pid = None;
        Ok(())
    }
}
