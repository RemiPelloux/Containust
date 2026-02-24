//! Runtime engine that orchestrates container lifecycle.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use containust_common::error::{ContainustError, Result};
use containust_common::types::ContainerId;

use crate::backend::{self, ContainerBackend, ContainerConfig, ContainerInfo};
use crate::exec::ExecOutput;

/// Information about a deployed component.
#[derive(Debug, Clone)]
pub struct DeployedComponent {
    /// Container ID assigned by the backend.
    pub id: ContainerId,
    /// Component name from the `.ctst` file.
    pub name: String,
    /// Exposed port, if any.
    pub port: Option<u16>,
    /// PID of the running process inside the backend.
    pub pid: Option<u32>,
}

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
            data_dir: containust_common::constants::data_dir().clone(),
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
    /// Creates a project-local `.containust/` directory next to the
    /// `.ctst` file for state and logs. Parses the composition,
    /// resolves dependencies, creates containers, and starts them.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing, validation, graph resolution,
    /// container creation, or start fails.
    pub fn deploy(&self, ctst_path: &Path) -> Result<Vec<DeployedComponent>> {
        let project_dir = containust_common::constants::project_dir(ctst_path);
        let _ = std::fs::create_dir_all(project_dir.join("logs"));
        let _ = std::fs::create_dir_all(project_dir.join("state"));
        tracing::info!(project_dir = %project_dir.display(), "project directory");

        let content = std::fs::read_to_string(ctst_path).map_err(|e| ContainustError::Io {
            path: ctst_path.to_path_buf(),
            source: e,
        })?;

        let composition = containust_compose::parser::parse_ctst(&content)?;
        let order = resolve_deploy_order(&composition)?;
        let resolved = containust_compose::resolver::resolve_connections(&composition)?;

        let mut deployed = Vec::new();
        for name in &order {
            if let Some(dc) = self.deploy_component(name, &composition, &resolved)? {
                deployed.push(dc);
            }
        }
        Ok(deployed)
    }

    /// Deploys a single named component from the composition.
    fn deploy_component(
        &self,
        name: &str,
        composition: &containust_compose::parser::ast::CompositionFile,
        resolved: &[containust_compose::resolver::ResolvedComponent],
    ) -> Result<Option<DeployedComponent>> {
        let Some(comp) = composition.components.iter().find(|c| c.name == name) else {
            return Ok(None);
        };
        let resolved_comp = resolved.iter().find(|r| r.name == name);

        let config = ContainerConfig {
            name: comp.name.clone(),
            image: comp.image.clone().unwrap_or_default(),
            command: comp.command.clone(),
            env: resolved_comp.map_or_else(Vec::new, |r| r.env.clone()),
            memory_bytes: comp.memory.as_deref().and_then(parse_memory),
            cpu_shares: comp.cpu.as_deref().and_then(|s| s.parse().ok()),
            readonly_rootfs: comp.readonly.unwrap_or(false),
            volumes: comp.volumes.clone(),
            port: comp.port,
        };

        eprintln!("  Creating container '{}'...", comp.name);
        let id = self.backend.create(&config)?;
        tracing::info!(id = %id, name = %comp.name, "container created");

        eprintln!("  Starting container '{}'...", comp.name);
        let pid = self.backend.start(&id)?;
        tracing::info!(id = %id, pid, name = %comp.name, "container started");

        Ok(Some(DeployedComponent {
            id,
            name: comp.name.clone(),
            port: comp.port,
            pid: Some(pid),
        }))
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

/// Builds a dependency graph and returns the topological ordering.
fn resolve_deploy_order(
    composition: &containust_compose::parser::ast::CompositionFile,
) -> Result<Vec<String>> {
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
    Ok(order)
}

/// Parses memory strings like "128MiB", "256MB", "1GiB" into bytes.
#[allow(clippy::option_if_let_else)]
fn parse_memory(s: &str) -> Option<u64> {
    let s = s.trim();
    let (num_str, multiplier) = if let Some(n) = s.strip_suffix("GiB") {
        (n, 1024 * 1024 * 1024)
    } else if let Some(n) = s.strip_suffix("GB") {
        (n, 1_000_000_000)
    } else if let Some(n) = s.strip_suffix("MiB") {
        (n, 1024 * 1024)
    } else if let Some(n) = s.strip_suffix("MB") {
        (n, 1_000_000)
    } else if let Some(n) = s.strip_suffix("KiB") {
        (n, 1024)
    } else if let Some(n) = s.strip_suffix("KB") {
        (n, 1000)
    } else {
        (s, 1)
    };
    num_str.trim().parse::<u64>().ok().map(|n| n * multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_memory_mib() {
        assert_eq!(parse_memory("128MiB"), Some(128 * 1024 * 1024));
    }

    #[test]
    fn parse_memory_gib() {
        assert_eq!(parse_memory("1GiB"), Some(1024 * 1024 * 1024));
    }

    #[test]
    fn parse_memory_plain_bytes() {
        assert_eq!(parse_memory("1048576"), Some(1_048_576));
    }

    #[test]
    fn parse_memory_invalid() {
        assert_eq!(parse_memory("abc"), None);
    }
}
