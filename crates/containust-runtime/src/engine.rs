//! Runtime engine that orchestrates container lifecycle.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use containust_common::codes;
use containust_common::error::{ContainustError, Result};
use containust_common::types::ContainerId;

use crate::backend::{
    self, ContainerBackend, ContainerConfig, ContainerInfo, ReconciliationReport,
};
use crate::events::{EventBus, OperationEmit};
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
    events: Arc<EventBus>,
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
            options.offline,
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
            events: Arc::new(EventBus::new()),
        }
    }

    /// Returns the shared lifecycle event bus.
    #[must_use]
    pub fn events(&self) -> &EventBus {
        &self.events
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
        let started = Instant::now();
        let project = self
            .data_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("project")
            .to_string();
        match self.deploy_inner(ctst_path) {
            Ok(deployed) => {
                self.events.emit_operation(OperationEmit {
                    project,
                    operation: "deploy".into(),
                    duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    container_id: None,
                    error_code: None,
                });
                Ok(deployed)
            }
            Err(error) => {
                let class = codes::classify(&error);
                self.events.emit_operation(OperationEmit {
                    project,
                    operation: "deploy".into(),
                    duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    container_id: None,
                    error_code: Some(class.code),
                });
                Err(error)
            }
        }
    }

    fn deploy_inner(&self, ctst_path: &Path) -> Result<Vec<DeployedComponent>> {
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
            let ports = published_ports(component, &composition.exposes)?;
            deployed.push(self.deploy_component(
                component,
                resolved_by_name.get(name.as_str()).copied(),
                ports,
            )?);
        }
        Ok(deployed)
    }

    /// Deploys a single named component from the composition.
    fn deploy_component(
        &self,
        comp: &containust_compose::parser::ast::ComponentDecl,
        resolved_comp: Option<&containust_compose::resolver::ResolvedComponent>,
        ports: Vec<u16>,
    ) -> Result<DeployedComponent> {
        validate_runtime_component(comp)?;
        let memory_bytes = parse_optional_memory(comp.memory.as_deref())?;
        let cpu_shares = parse_optional_cpu(comp.cpu.as_deref())?;
        let restart = parse_restart_policy(comp)?;
        let healthcheck = comp
            .healthcheck
            .as_ref()
            .map(|decl| parse_healthcheck_spec(&comp.name, decl))
            .transpose()?;

        let image = resolve_deploy_image(self.data_dir(), self.offline, comp)?;
        let mut namespaces = containust_core::namespace::NamespaceConfig::default();
        if !ports.is_empty() {
            // Published ports share the host network namespace (identity
            // mapping), like `docker run --network host`. veth/NAT-based
            // publishing is deferred — see docs/SUPPORT_POLICY.md.
            namespaces.network = false;
        }
        let config = ContainerConfig {
            name: comp.name.clone(),
            image,
            command: effective_command(comp),
            env: resolved_comp.map_or_else(Vec::new, |r| r.env.clone()),
            memory_bytes,
            cpu_shares,
            readonly_rootfs: comp.readonly.unwrap_or(true),
            volumes: component_volumes(comp),
            port: comp.port,
            ports,
            restart,
            healthcheck,
            namespaces,
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
        self.stop_with_force(id, false)
    }

    /// Stops a container immediately when `force` is true.
    ///
    /// # Errors
    ///
    /// Returns an error if the container cannot be stopped.
    pub fn stop_with_force(&self, id: &ContainerId, force: bool) -> Result<()> {
        let started = Instant::now();
        let project = self
            .data_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("project")
            .to_string();
        let result = if force {
            self.backend.force_stop(id)
        } else {
            self.backend.stop(id)
        };
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        match &result {
            Ok(()) => self.events.emit_operation(OperationEmit {
                project,
                operation: "stop".into(),
                duration_ms,
                container_id: Some(id.clone()),
                error_code: None,
            }),
            Err(error) => {
                let class = codes::classify(error);
                self.events.emit_operation(OperationEmit {
                    project,
                    operation: "stop".into(),
                    duration_ms,
                    container_id: Some(id.clone()),
                    error_code: Some(class.code),
                });
            }
        }
        result
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
    /// Boots a lightweight Alpine Linux VM via QEMU. Custom kernel and
    /// initramfs paths are not yet supported and fail closed.
    ///
    /// # Errors
    ///
    /// Returns an error if QEMU is not installed, custom assets are
    /// requested, or the VM fails to start.
    pub fn vm_start(&self, kernel: Option<&str>, initramfs: Option<&str>) -> Result<()> {
        if kernel.is_some() || initramfs.is_some() {
            return Err(ContainustError::Config {
                message: "custom --kernel/--initramfs is not supported yet; \
                     omit them to use pinned Alpine netboot assets in \
                     ~/.containust/cache/vm/"
                    .into(),
            });
        }
        let Some(vm) = self
            .backend
            .as_any()
            .downcast_ref::<crate::backend::vm::VMBackend>()
        else {
            return Err(ContainustError::Config {
                message: "VM backend is only available on macOS/Windows".into(),
            });
        };

        vm.ensure_vm_running(&[])
    }

    /// Stops the QEMU-based VM backend.
    ///
    /// Without `force`, sends SIGTERM and escalates to SIGKILL after a
    /// short grace period. With `force`, sends SIGKILL immediately.
    /// Idempotent when the VM is already stopped.
    ///
    /// # Errors
    ///
    /// Returns an error if stop cannot be completed safely.
    pub fn vm_stop(&self, force: bool) -> Result<()> {
        let Some(vm) = self
            .backend
            .as_any()
            .downcast_ref::<crate::backend::vm::VMBackend>()
        else {
            return Err(ContainustError::Config {
                message: "VM backend is only available on macOS/Windows".into(),
            });
        };

        vm.stop_vm(force)
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

/// Resolves `preset://` images into catalog references before create.
fn resolve_deploy_image(
    data_dir: &Path,
    offline: bool,
    comp: &containust_compose::parser::ast::ComponentDecl,
) -> Result<String> {
    let Some(image) = comp.image.as_deref() else {
        return Ok(String::new());
    };
    let reference = containust_image::reference::ImageReference::parse(image)?;
    if reference.scheme() != containust_image::reference::ImageScheme::Preset {
        return Ok(image.to_string());
    }
    let request = containust_image::import::ImportRequest::new(&comp.name, offline);
    let entry = containust_image::import::import_image(data_dir, &reference, &request)?;
    let digest = entry.digest.as_deref().unwrap_or_default();
    Ok(format!("image://{}@sha256:{digest}", entry.name))
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

/// Merges a component's `ports` list with `EXPOSE` statements targeting it.
///
/// The runtime publishes ports identically on host and container; an
/// `EXPOSE` with differing host and container ports fails closed.
///
/// # Errors
///
/// Returns an error for host/container remapping, which is unsupported.
fn published_ports(
    comp: &containust_compose::parser::ast::ComponentDecl,
    exposes: &[containust_compose::parser::ast::ExposeDecl],
) -> Result<Vec<u16>> {
    let declared: std::collections::HashSet<u16> =
        comp.port.iter().chain(comp.ports.iter()).copied().collect();
    let mut ports = comp.ports.clone();
    for expose in exposes {
        if !declared.contains(&expose.container_port) {
            continue;
        }
        if expose.host_port != expose.container_port {
            return Err(ContainustError::Config {
                message: format!(
                    "EXPOSE {}:{} remaps host port {} to container port {}, which the \
                     runtime does not support yet. Use `EXPOSE {}` (identity mapping) \
                     or have the component listen on port {} directly",
                    expose.host_port,
                    expose.container_port,
                    expose.host_port,
                    expose.container_port,
                    expose.container_port,
                    expose.host_port
                ),
            });
        }
        if !ports.contains(&expose.container_port) {
            ports.push(expose.container_port);
        }
    }
    Ok(ports)
}

fn parse_restart_policy(
    component: &containust_compose::parser::ast::ComponentDecl,
) -> Result<containust_common::types::RestartPolicy> {
    component.restart.as_deref().map_or_else(
        || Ok(containust_common::types::RestartPolicy::Never),
        |value| {
            containust_common::types::RestartPolicy::parse(value).map_err(|message| {
                ContainustError::Config {
                    message: format!("component '{}': {message}", component.name),
                }
            })
        },
    )
}

fn parse_healthcheck_spec(
    component_name: &str,
    decl: &containust_compose::parser::ast::HealthcheckDecl,
) -> Result<containust_common::types::HealthcheckSpec> {
    if decl.command.is_empty() {
        return Err(ContainustError::Config {
            message: format!("component '{component_name}': healthcheck command is empty"),
        });
    }
    let defaults = containust_common::types::HealthcheckSpec::default();
    Ok(containust_common::types::HealthcheckSpec {
        command: decl.command.clone(),
        interval_secs: parse_healthcheck_duration(
            component_name,
            decl.interval.as_deref(),
            defaults.interval_secs,
        )?,
        timeout_secs: parse_healthcheck_duration(
            component_name,
            decl.timeout.as_deref(),
            defaults.timeout_secs,
        )?,
        retries: decl.retries.unwrap_or(defaults.retries),
        start_period_secs: parse_healthcheck_duration(
            component_name,
            decl.start_period.as_deref(),
            defaults.start_period_secs,
        )?,
    })
}

fn parse_healthcheck_duration(
    component_name: &str,
    value: Option<&str>,
    default_secs: u64,
) -> Result<u64> {
    let Some(text) = value else {
        return Ok(default_secs);
    };
    parse_duration_secs(text).ok_or_else(|| ContainustError::Config {
        message: format!(
            "component '{component_name}': invalid healthcheck duration '{text}' \
             (expected e.g. \"30s\", \"1m\", \"1h\")"
        ),
    })
}

/// Parses `"30s"`, `"5m"`, `"1h"`, or a plain seconds integer.
fn parse_duration_secs(text: &str) -> Option<u64> {
    const UNITS: [(char, u64); 3] = [('h', 3600), ('m', 60), ('s', 1)];
    let text = text.trim();
    let (digits, multiplier) = UNITS
        .iter()
        .find_map(|&(suffix, mult)| text.strip_suffix(suffix).map(|digits| (digits, mult)))
        .unwrap_or((text, 1));
    digits.trim().parse::<u64>().ok().map(|n| n * multiplier)
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
        let file = dir.path().join("workdir.ctst");
        std::fs::write(
            &file,
            r#"COMPONENT app {
    image = "file:///unused"
    workdir = "/srv"
}"#,
        )
        .expect("write composition");
        let state = Arc::new(FakeState::default());
        let engine = fake_engine(Arc::clone(&state), dir.path().join("data"), false);

        let error = engine.deploy(&file).expect_err("unsupported property");
        assert!(error.to_string().contains("workdir"));
        assert!(state.config.lock().expect("config lock").is_none());
    }

    #[test]
    fn deploy_passes_ports_restart_and_healthcheck() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("policies.ctst");
        std::fs::write(
            &file,
            r#"COMPONENT app {
    image = "file:///unused"
    ports = [8080, 9090]
    restart = "on-failure"
    healthcheck = {
        command = ["curl", "-f", "http://localhost:8080/healthz"]
        interval = "10s"
        timeout = "3s"
        retries = 5
        start_period = "1m"
    }
}"#,
        )
        .expect("write composition");
        let state = Arc::new(FakeState::default());
        let engine = fake_engine(Arc::clone(&state), dir.path().join("data"), false);

        let _ = engine.deploy(&file).expect("deploy");
        let config = state
            .config
            .lock()
            .expect("config lock")
            .clone()
            .expect("config captured");

        assert_eq!(config.ports, vec![8080, 9090]);
        assert_eq!(
            config.restart,
            containust_common::types::RestartPolicy::OnFailure
        );
        let healthcheck = config.healthcheck.expect("healthcheck spec");
        assert_eq!(healthcheck.command[0], "curl");
        assert_eq!(healthcheck.interval_secs, 10);
        assert_eq!(healthcheck.timeout_secs, 3);
        assert_eq!(healthcheck.retries, 5);
        assert_eq!(healthcheck.start_period_secs, 60);
    }

    #[test]
    fn deploy_rejects_invalid_restart_policy_value() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("restart.ctst");
        std::fs::write(
            &file,
            r#"COMPONENT app {
    image = "file:///unused"
    restart = "sometimes"
}"#,
        )
        .expect("write composition");
        let state = Arc::new(FakeState::default());
        let engine = fake_engine(Arc::clone(&state), dir.path().join("data"), false);

        let error = engine.deploy(&file).expect_err("invalid restart");
        assert!(error.to_string().contains("restart policy"));
        assert!(state.config.lock().expect("config lock").is_none());
    }

    #[test]
    fn deploy_expose_identity_publishes_port() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("expose.ctst");
        std::fs::write(
            &file,
            r#"COMPONENT web {
    image = "file:///unused"
    port = 3000
}
EXPOSE 3000"#,
        )
        .expect("write composition");
        let state = Arc::new(FakeState::default());
        let engine = fake_engine(Arc::clone(&state), dir.path().join("data"), false);

        let _ = engine.deploy(&file).expect("deploy");
        let config = state
            .config
            .lock()
            .expect("config lock")
            .clone()
            .expect("config captured");
        assert_eq!(config.ports, vec![3000]);
        // Published ports share the host network namespace on Linux.
        assert!(!config.namespaces.network);
    }

    #[test]
    fn deploy_expose_remap_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("remap.ctst");
        std::fs::write(
            &file,
            r#"COMPONENT web {
    image = "file:///unused"
    port = 8080
}
EXPOSE 80:8080"#,
        )
        .expect("write composition");
        let state = Arc::new(FakeState::default());
        let engine = fake_engine(Arc::clone(&state), dir.path().join("data"), false);

        let error = engine.deploy(&file).expect_err("remap unsupported");
        assert!(error.to_string().contains("does not support"));
        assert!(state.config.lock().expect("config lock").is_none());
    }

    #[test]
    fn deploy_healthcheck_example_no_longer_errors() {
        let example =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/healthcheck_example.ctst");
        let dir = tempfile::tempdir().expect("tempdir");
        let state = Arc::new(FakeState::default());
        let engine = fake_engine(Arc::clone(&state), dir.path().join("data"), false);

        let deployed = engine.deploy(&example).expect("example deploys");
        assert!(!deployed.is_empty());
    }

    #[test]
    fn parse_duration_secs_supports_units() {
        assert_eq!(parse_duration_secs("30s"), Some(30));
        assert_eq!(parse_duration_secs("5m"), Some(300));
        assert_eq!(parse_duration_secs("1h"), Some(3600));
        assert_eq!(parse_duration_secs("45"), Some(45));
        assert_eq!(parse_duration_secs("abc"), None);
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
