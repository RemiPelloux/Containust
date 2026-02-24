//! Runtime engine that orchestrates container lifecycle.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use containust_common::error::{ContainustError, Result};
use containust_common::types::ContainerId;

use crate::backend::{self, ContainerBackend, ContainerConfig, ContainerInfo};
use crate::exec::ExecOutput;

/// The runtime engine that coordinates all container operations.
///
/// Provides a high-level API that delegates to the platform-specific
/// backend and integrates with the compose layer for `.ctst` deployments.
pub struct Engine {
    backend: Box<dyn ContainerBackend>,
    data_dir: PathBuf,
}

impl Engine {
    /// Creates a new engine with auto-detected platform backend.
    #[must_use]
    pub fn new() -> Self {
        Self {
            backend: backend::detect_backend(),
            data_dir: PathBuf::from(containust_common::constants::DEFAULT_DATA_DIR),
        }
    }

    /// Creates a new engine with a custom data directory.
    #[must_use]
    pub fn with_data_dir(data_dir: PathBuf) -> Self {
        Self {
            backend: backend::detect_backend(),
            data_dir,
        }
    }

    /// Deploys all components from a `.ctst` file.
    ///
    /// Parses the composition file, resolves dependency ordering,
    /// auto-wires environment variables, and creates containers
    /// in topological order.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing, validation, graph resolution,
    /// or container creation fails.
    pub fn deploy(&self, ctst_path: &Path) -> Result<Vec<ContainerId>> {
        let content = std::fs::read_to_string(ctst_path).map_err(|e| ContainustError::Io {
            path: ctst_path.to_path_buf(),
            source: e,
        })?;

        let composition = containust_compose::parser::parse_ctst(&content)?;

        let mut graph = containust_compose::graph::DependencyGraph::new();
        let mut node_map = HashMap::new();
        for comp in &composition.components {
            let idx = graph.add_component(&comp.name);
            let _ = node_map.insert(comp.name.clone(), idx);
        }
        for conn in &composition.connections {
            if let (Some(&from), Some(&to)) = (node_map.get(&conn.from), node_map.get(&conn.to)) {
                graph.add_dependency(from, to);
            }
        }

        let order = graph.resolve_order()?;
        tracing::info!(?order, "deployment order resolved");

        let resolved = containust_compose::resolver::resolve_connections(&composition)?;

        let mut ids = Vec::new();
        for name in &order {
            let comp = composition.components.iter().find(|c| &c.name == name);
            let resolved_comp = resolved.iter().find(|r| &r.name == name);

            if let Some(comp) = comp {
                let config = ContainerConfig {
                    name: comp.name.clone(),
                    image: comp.image.clone().unwrap_or_default(),
                    command: comp.command.clone(),
                    env: resolved_comp.map_or_else(Vec::new, |r| r.env.clone()),
                    memory_bytes: None,
                    cpu_shares: None,
                    readonly_rootfs: comp.readonly.unwrap_or(true),
                    volumes: comp.volumes.clone(),
                    port: comp.port,
                };

                let id = self.backend.create(&config)?;
                tracing::info!(id = %id, name = %comp.name, "container created");
                ids.push(id);
            }
        }

        Ok(ids)
    }

    /// Lists all containers.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend cannot retrieve state.
    pub fn list(&self) -> Result<Vec<ContainerInfo>> {
        self.backend.list()
    }

    /// Stops a container by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the container is not found or cannot be stopped.
    pub fn stop(&self, id: &ContainerId) -> Result<()> {
        self.backend.stop(id)
    }

    /// Stops all running containers.
    ///
    /// # Errors
    ///
    /// Returns an error if any container cannot be stopped.
    pub fn stop_all(&self) -> Result<()> {
        let containers = self.backend.list()?;
        for info in containers {
            if info.state == "running" {
                self.backend.stop(&info.id)?;
            }
        }
        Ok(())
    }

    /// Executes a command inside a running container.
    ///
    /// # Errors
    ///
    /// Returns an error if the container is not running or the
    /// command fails to execute.
    pub fn exec(&self, id: &ContainerId, cmd: &[String]) -> Result<ExecOutput> {
        self.backend.exec(id, cmd)
    }

    /// Returns the logs for a container.
    ///
    /// # Errors
    ///
    /// Returns an error if logs cannot be retrieved.
    pub fn logs(&self, id: &ContainerId) -> Result<String> {
        self.backend.logs(id)
    }

    /// Returns the data directory path.
    #[must_use]
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Returns whether the backend is operational on this platform.
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.backend.is_available()
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
