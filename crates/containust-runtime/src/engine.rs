//! Runtime engine that orchestrates container lifecycle.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use containust_common::error::{ContainustError, Result};
use containust_common::types::ContainerId;

use crate::backend::{
    self, ContainerBackend, ContainerConfig, ContainerInfo, ReconciliationReport,
};
use crate::exec::ExecOutput;

/// Immutable storage and network policy for an engine instance.
#[derive(Debug, Clone)]
pub struct EngineOptions {
    /// Directory for rootfs, logs, and images.
    pub data_dir: PathBuf,
    /// JSON state index path.
    pub state_file: PathBuf,
    /// Whether remote sources are rejected.
    pub offline: bool,
}

impl Default for EngineOptions {
    fn default() -> Self {
        let data_dir = containust_common::constants::project_dir(Path::new("containust.ctst"));
        Self {
            state_file: data_dir.join("state").join("state.json"),
            data_dir,
            offline: false,
        }
    }
}

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
    state_file: PathBuf,
    offline: bool,
}

impl Engine {
    /// Creates a new engine with auto-detected platform backend.
    #[must_use]
    pub fn new() -> Self {
        Self::with_options(EngineOptions::default())
    }

    /// Creates a new engine with a custom data directory.
    #[must_use]
    pub fn with_data_dir(data_dir: PathBuf) -> Self {
        let state_file = data_dir.join("state").join("state.json");
        Self::with_options(EngineOptions {
            data_dir,
            state_file,
            offline: false,
        })
    }

    /// Creates an engine with explicit storage and network policy.
    #[must_use]
    pub fn with_options(options: EngineOptions) -> Self {
        let backend = backend::detect_backend_with_paths(
            options.data_dir.clone(),
            options.state_file.clone(),
        );
        Self::with_backend(options, backend)
    }

    /// Creates an engine with an explicitly supplied backend.
    #[must_use]
    pub fn with_backend(options: EngineOptions, backend: Box<dyn ContainerBackend>) -> Self {
        Self {
            backend,
            data_dir: options.data_dir,
            state_file: options.state_file,
            offline: options.offline,
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
        for subdir in ["logs", "state"] {
            let path = project_dir.join(subdir);
            std::fs::create_dir_all(&path)
                .map_err(|source| ContainustError::Io { path, source })?;
        }
        tracing::info!(project_dir = %project_dir.display(), "project directory");

        let content = std::fs::read_to_string(ctst_path).map_err(|e| ContainustError::Io {
            path: ctst_path.to_path_buf(),
            source: e,
        })?;

        let composition = containust_compose::parser::parse_ctst(&content)?;
        if self.offline {
            containust_compose::validate_offline(&composition)?;
        }
        let order = resolve_deploy_order(&composition)?;
        let resolved = containust_compose::resolver::resolve_connections(&composition)?;
        let components: HashMap<&str, &containust_compose::parser::ast::ComponentDecl> =
            composition
                .components
                .iter()
                .map(|component| (component.name.as_str(), component))
                .collect();
        let resolved_by_name: HashMap<&str, &containust_compose::resolver::ResolvedComponent> =
            resolved
                .iter()
                .map(|component| (component.name.as_str(), component))
                .collect();

        let mut deployed = Vec::with_capacity(order.len());
        for name in &order {
            let component =
                components
                    .get(name.as_str())
                    .ok_or_else(|| ContainustError::NotFound {
                        kind: "component",
                        id: name.clone(),
                    })?;
            deployed.push(
                self.deploy_component(component, resolved_by_name.get(name.as_str()).copied())?,
            );
        }
        Ok(deployed)
    }

    /// Deploys a single named component from the composition.
    fn deploy_component(
        &self,
        comp: &containust_compose::parser::ast::ComponentDecl,
        resolved_comp: Option<&containust_compose::resolver::ResolvedComponent>,
    ) -> Result<DeployedComponent> {
        validate_runtime_component(comp)?;
        let memory_bytes = parse_optional_memory(comp.memory.as_deref())?;
        let cpu_shares = parse_optional_cpu(comp.cpu.as_deref())?;

        let config = ContainerConfig {
            name: comp.name.clone(),
            image: comp.image.clone().unwrap_or_default(),
            command: effective_command(comp),
            env: resolved_comp.map_or_else(Vec::new, |r| r.env.clone()),
            memory_bytes,
            cpu_shares,
            readonly_rootfs: comp.readonly.unwrap_or(true),
            volumes: component_volumes(comp),
            port: comp.port,
        };

        eprintln!("  Creating container '{}'...", comp.name);
        let id = self.backend.create(&config)?;
        tracing::info!(id = %id, name = %comp.name, "container created");

        eprintln!("  Starting container '{}'...", comp.name);
        let pid = self.backend.start(&id)?;
        tracing::info!(id = %id, pid, name = %comp.name, "container started");

        Ok(DeployedComponent {
            id,
            name: comp.name.clone(),
            port: comp.port,
            pid: Some(pid),
        })
    }

    /// Lists all containers.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend cannot retrieve state.
    pub fn list(&self) -> Result<Vec<ContainerInfo>> {
        self.list_reconciled().map(|(containers, _)| containers)
    }

    /// Lists containers and returns the reconciliation work performed first.
    ///
    /// # Errors
    ///
    /// Returns an error if reconciliation or state loading fails.
    pub fn list_reconciled(&self) -> Result<(Vec<ContainerInfo>, ReconciliationReport)> {
        let report = self.backend.reconcile()?;
        if report != ReconciliationReport::default() {
            tracing::info!(?report, "runtime state reconciled");
        }
        Ok((self.backend.list()?, report))
    }

    /// Reconciles persisted state with live backend resources.
    ///
    /// # Errors
    ///
    /// Returns an error if persisted state cannot be inspected or repaired.
    pub fn reconcile(&self) -> Result<ReconciliationReport> {
        self.backend.reconcile()
    }

    /// Stops a container by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the container is not found or cannot be stopped.
    pub fn stop(&self, id: &ContainerId) -> Result<()> {
        self.backend.stop(id)
    }

    /// Stops a container immediately when `force` is true.
    ///
    /// # Errors
    ///
    /// Returns an error if the container cannot be stopped.
    pub fn stop_with_force(&self, id: &ContainerId, force: bool) -> Result<()> {
        if force {
            self.backend.force_stop(id)
        } else {
            self.backend.stop(id)
        }
    }

    /// Removes a stopped container and all project-owned resources.
    ///
    /// # Errors
    ///
    /// Returns an error if the container is running, missing, or cleanup fails.
    pub fn remove(&self, id: &ContainerId) -> Result<()> {
        self.backend.remove(id)
    }

    /// Stops all running containers.
    ///
    /// # Errors
    ///
    /// Returns an error if any container cannot be stopped.
    pub fn stop_all(&self) -> Result<()> {
        self.stop_all_with_force(false)
    }

    /// Stops all running containers, optionally skipping graceful shutdown.
    ///
    /// # Errors
    ///
    /// Returns an error if any container cannot be stopped.
    pub fn stop_all_with_force(&self, force: bool) -> Result<()> {
        let containers = self.backend.list()?;
        for info in containers {
            if info.state == "running" {
                self.stop_with_force(&info.id, force)?;
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

    /// Returns the configured state file path.
    #[must_use]
    pub fn state_file(&self) -> &Path {
        &self.state_file
    }

    /// Returns whether remote sources are blocked.
    #[must_use]
    pub const fn offline(&self) -> bool {
        self.offline
    }

    /// Returns whether the backend is operational on this platform.
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.backend.is_available()
    }

    /// Starts the QEMU-based VM backend on macOS/Windows.
    ///
    /// Boots a lightweight Alpine Linux VM via QEMU with optional
    /// custom kernel and initramfs paths. Does nothing on Linux
    /// (native backend).
    ///
    /// # Errors
    ///
    /// Returns an error if QEMU is not installed or the VM fails to start.
    pub fn vm_start(&self, kernel: Option<&str>, initramfs: Option<&str>) -> Result<()> {
        let Some(vm) = self
            .backend
            .as_any()
            .downcast_ref::<crate::backend::vm::VMBackend>()
        else {
            return Err(ContainustError::Config {
                message: "VM backend is only available on macOS/Windows".into(),
            });
        };

        _ = kernel;
        _ = initramfs;
        vm.ensure_vm_running(&[])
    }

    /// Stops the QEMU-based VM backend.
    ///
    /// Gracefully shuts down the Alpine Linux VM. The `force` flag
    /// is reserved for future use (currently both paths send SIGKILL).
    ///
    /// # Errors
    ///
    /// Returns an error if the VM is not running.
    pub fn vm_stop(&self, force: bool) -> Result<()> {
        _ = force;
        let Some(vm) = self
            .backend
            .as_any()
            .downcast_ref::<crate::backend::vm::VMBackend>()
        else {
            return Err(ContainustError::Config {
                message: "VM backend is only available on macOS/Windows".into(),
            });
        };

        vm.stop_vm()
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

fn component_volumes(component: &containust_compose::parser::ast::ComponentDecl) -> Vec<String> {
    component
        .volume
        .iter()
        .cloned()
        .chain(component.volumes.iter().cloned())
        .collect()
}

fn effective_command(component: &containust_compose::parser::ast::ComponentDecl) -> Vec<String> {
    component
        .entrypoint
        .iter()
        .flatten()
        .cloned()
        .chain(component.command.iter().cloned())
        .collect()
}

fn validate_runtime_component(
    component: &containust_compose::parser::ast::ComponentDecl,
) -> Result<()> {
    let unsupported = [
        (component.workdir.is_some(), "workdir"),
        (component.user.is_some(), "user"),
        (component.hostname.is_some(), "hostname"),
        (component.restart.is_some(), "restart"),
        (component.healthcheck.is_some(), "healthcheck"),
        (!component.ports.is_empty(), "ports"),
    ]
    .into_iter()
    .filter_map(|(present, name)| present.then_some(name))
    .collect::<Vec<_>>();
    if !unsupported.is_empty() {
        return Err(ContainustError::Config {
            message: format!(
                "component '{}' uses unsupported runtime properties: {}",
                component.name,
                unsupported.join(", ")
            ),
        });
    }
    if component
        .network
        .as_deref()
        .is_some_and(|mode| mode != "host")
    {
        return Err(ContainustError::Config {
            message: format!(
                "component '{}' requests unsupported network mode",
                component.name
            ),
        });
    }
    Ok(())
}

fn parse_optional_memory(value: Option<&str>) -> Result<Option<u64>> {
    value
        .map(|text| {
            parse_memory(text).ok_or_else(|| ContainustError::Config {
                message: format!("invalid memory limit: {text}"),
            })
        })
        .transpose()
}

fn parse_optional_cpu(value: Option<&str>) -> Result<Option<u64>> {
    value
        .map(|text| {
            parse_cpu_shares(text).ok_or_else(|| ContainustError::Config {
                message: format!("invalid CPU limit: {text}"),
            })
        })
        .transpose()
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

fn parse_cpu_shares(value: &str) -> Option<u64> {
    let value = value.trim();
    if let Ok(shares) = value.parse::<u64>() {
        return (1..=10_000).contains(&shares).then_some(shares);
    }
    let (whole_text, fraction_text) = value.split_once('.')?;
    if fraction_text.is_empty() || !fraction_text.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let whole = if whole_text.is_empty() {
        0
    } else {
        whole_text.parse::<u64>().ok()?
    };
    let fraction = fraction_text.parse::<u64>().ok()?;
    let scale = 10_u64.checked_pow(fraction_text.len().try_into().ok()?)?;
    let shares = whole
        .checked_mul(1024)?
        .checked_add(fraction.checked_mul(1024)?.checked_div(scale)?)?;
    Some(shares.clamp(1, 10_000))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[derive(Default)]
    struct FakeState {
        config: Mutex<Option<ContainerConfig>>,
        force_stopped: AtomicBool,
    }

    struct FakeBackend {
        state: Arc<FakeState>,
    }

    impl ContainerBackend for FakeBackend {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn create(&self, config: &ContainerConfig) -> Result<ContainerId> {
            *self.state.config.lock().expect("config lock") = Some(config.clone());
            Ok(ContainerId::new("fake-id"))
        }

        fn start(&self, _id: &ContainerId) -> Result<u32> {
            Ok(42)
        }

        fn stop(&self, _id: &ContainerId) -> Result<()> {
            Ok(())
        }

        fn force_stop(&self, _id: &ContainerId) -> Result<()> {
            self.state.force_stopped.store(true, Ordering::Release);
            Ok(())
        }

        fn exec(&self, _id: &ContainerId, _cmd: &[String]) -> Result<ExecOutput> {
            Ok(ExecOutput {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 0,
            })
        }

        fn remove(&self, _id: &ContainerId) -> Result<()> {
            Ok(())
        }

        fn logs(&self, _id: &ContainerId) -> Result<String> {
            Ok(String::new())
        }

        fn list(&self) -> Result<Vec<ContainerInfo>> {
            Ok(Vec::new())
        }

        fn is_available(&self) -> bool {
            true
        }
    }

    fn fake_engine(state: Arc<FakeState>, data_dir: PathBuf, offline: bool) -> Engine {
        let options = EngineOptions {
            state_file: data_dir.join("custom-state.json"),
            data_dir,
            offline,
        };
        Engine::with_backend(options, Box::new(FakeBackend { state }))
    }

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

    #[test]
    fn parse_cpu_decimal_maps_to_weight() {
        assert_eq!(parse_cpu_shares("0.5"), Some(512));
        assert_eq!(parse_cpu_shares("2"), Some(2));
        assert_eq!(parse_cpu_shares("0"), None);
        assert_eq!(parse_cpu_shares("invalid"), None);
    }

    #[test]
    fn engine_preserves_explicit_options() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = Arc::new(FakeState::default());
        let engine = fake_engine(Arc::clone(&state), dir.path().to_path_buf(), true);

        assert_eq!(engine.data_dir(), dir.path());
        assert_eq!(engine.state_file(), dir.path().join("custom-state.json"));
        assert!(engine.offline());
    }

    #[test]
    fn deploy_passes_full_component_configuration() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("app.ctst");
        std::fs::write(
            &file,
            r#"COMPONENT app {
    image = "file:///unused"
    entrypoint = ["/bin/app"]
    command = ["--serve"]
    cpu = "0.5"
    memory = "64MiB"
    volume = "/tmp:/data:ro"
    env = { MODE = "test" }
}"#,
        )
        .expect("write composition");
        let state = Arc::new(FakeState::default());
        let engine = fake_engine(Arc::clone(&state), dir.path().join("data"), false);

        let deployed = engine.deploy(&file).expect("deploy");
        let config = state
            .config
            .lock()
            .expect("config lock")
            .clone()
            .expect("config captured");

        assert_eq!(deployed.len(), 1);
        assert_eq!(config.command, vec!["/bin/app", "--serve"]);
        assert_eq!(config.cpu_shares, Some(512));
        assert_eq!(config.memory_bytes, Some(64 * 1024 * 1024));
        assert!(config.readonly_rootfs);
        assert_eq!(config.volumes, vec!["/tmp:/data:ro"]);
        assert_eq!(config.env, vec![("MODE".into(), "test".into())]);
    }

    #[test]
    fn offline_deploy_rejects_remote_image_before_create() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("remote.ctst");
        std::fs::write(
            &file,
            "COMPONENT app { image = \"https://example.test/app.tar\" }",
        )
        .expect("write composition");
        let state = Arc::new(FakeState::default());
        let engine = fake_engine(Arc::clone(&state), dir.path().join("data"), true);

        assert!(engine.deploy(&file).is_err());
        assert!(state.config.lock().expect("config lock").is_none());
    }

    #[test]
    fn deploy_rejects_unsupported_runtime_property() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("health.ctst");
        std::fs::write(
            &file,
            r#"COMPONENT app {
    image = "file:///unused"
    healthcheck = { command = ["true"] }
}"#,
        )
        .expect("write composition");
        let state = Arc::new(FakeState::default());
        let engine = fake_engine(Arc::clone(&state), dir.path().join("data"), false);

        let error = engine.deploy(&file).expect_err("unsupported property");
        assert!(error.to_string().contains("healthcheck"));
        assert!(state.config.lock().expect("config lock").is_none());
    }

    #[test]
    fn deploy_rejects_invalid_resource_value() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("invalid-memory.ctst");
        std::fs::write(
            &file,
            r#"COMPONENT app {
    image = "file:///unused"
    memory = "a lot"
}"#,
        )
        .expect("write composition");
        let state = Arc::new(FakeState::default());
        let engine = fake_engine(Arc::clone(&state), dir.path().join("data"), false);

        let error = engine.deploy(&file).expect_err("invalid memory");
        assert!(error.to_string().contains("invalid memory"));
        assert!(state.config.lock().expect("config lock").is_none());
    }

    #[test]
    fn forced_stop_uses_backend_fast_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = Arc::new(FakeState::default());
        let engine = fake_engine(Arc::clone(&state), dir.path().to_path_buf(), false);

        engine
            .stop_with_force(&ContainerId::new("fake-id"), true)
            .expect("force stop");
        assert!(state.force_stopped.load(Ordering::Acquire));
    }
}
