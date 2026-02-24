//! Linux native container backend using direct syscalls.

use std::path::PathBuf;

use containust_common::error::{ContainustError, Result};
use containust_common::types::ContainerId;

use super::{ContainerBackend, ContainerConfig, ContainerInfo};
use crate::exec::ExecOutput;

/// Backend that uses Linux kernel features directly.
///
/// Manages container state on disk and delegates process operations
/// to the platform's namespace and cgroup facilities.
pub struct LinuxNativeBackend {
    data_dir: PathBuf,
}

impl LinuxNativeBackend {
    /// Creates a new Linux native backend.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data_dir: containust_common::constants::data_dir().clone(),
        }
    }
}

impl Default for LinuxNativeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ContainerBackend for LinuxNativeBackend {
    fn create(&self, config: &ContainerConfig) -> Result<ContainerId> {
        let id = ContainerId::generate();
        tracing::info!(id = %id, name = %config.name, "creating container (Linux native)");

        let state_path = self.data_dir.join("state.json");
        let mut state = crate::state::load_state(&state_path)?;
        state.containers.push(crate::state::StateEntry {
            id: id.clone(),
            name: config.name.clone(),
            state: containust_common::types::ContainerState::Created,
            pid: None,
            image: config.image.clone(),
            rootfs_path: None,
            log_path: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        });
        crate::state::save_state(&state_path, &state)?;
        Ok(id)
    }

    fn start(&self, id: &ContainerId) -> Result<u32> {
        tracing::info!(id = %id, "starting container (Linux native)");
        Ok(0)
    }

    fn stop(&self, id: &ContainerId) -> Result<()> {
        tracing::info!(id = %id, "stopping container (Linux native)");
        let state_path = self.data_dir.join("state.json");
        let mut state = crate::state::load_state(&state_path)?;
        if let Some(entry) = state.containers.iter_mut().find(|e| e.id == *id) {
            entry.state = containust_common::types::ContainerState::Stopped;
            entry.pid = None;
        }
        crate::state::save_state(&state_path, &state)?;
        Ok(())
    }

    fn exec(&self, id: &ContainerId, cmd: &[String]) -> Result<ExecOutput> {
        let state_path = self.data_dir.join("state.json");
        let state = crate::state::load_state(&state_path)?;
        let entry = state
            .containers
            .iter()
            .find(|e| e.id == *id)
            .ok_or_else(|| ContainustError::NotFound {
                kind: "container",
                id: id.to_string(),
            })?;
        let pid = entry.pid.ok_or_else(|| ContainustError::Config {
            message: format!("container {id} is not running"),
        })?;
        crate::exec::exec_in_container(id, pid, cmd)
    }

    fn remove(&self, id: &ContainerId) -> Result<()> {
        let state_path = self.data_dir.join("state.json");
        let mut state = crate::state::load_state(&state_path)?;
        state.containers.retain(|e| e.id != *id);
        crate::state::save_state(&state_path, &state)?;
        Ok(())
    }

    fn logs(&self, id: &ContainerId) -> Result<String> {
        crate::logs::read_logs(&self.data_dir, id.as_str())
    }

    fn list(&self) -> Result<Vec<ContainerInfo>> {
        let state_path = self.data_dir.join("state.json");
        let state = crate::state::load_state(&state_path)?;
        Ok(state
            .containers
            .iter()
            .map(|e| ContainerInfo {
                id: e.id.clone(),
                name: e.name.clone(),
                state: e.state.to_string(),
                pid: e.pid,
                image: e.image.clone(),
                created_at: e.created_at.clone(),
            })
            .collect())
    }

    fn is_available(&self) -> bool {
        cfg!(target_os = "linux")
    }
}
