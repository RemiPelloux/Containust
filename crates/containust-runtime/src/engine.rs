//! Runtime engine that orchestrates container lifecycle.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use containust_common::codes;
use containust_common::error::{ContainustError, Result};
use containust_common::types::{ContainerId, UpdateStrategy};

use crate::backend::{
    self, ContainerBackend, ContainerConfig, ContainerInfo, ReconciliationReport,
};
use crate::events::{EventBus, OperationEmit};
use crate::exec::ExecOutput;
use crate::plan::{
    AppliedComponent, DeployPlan, DesiredComponent, Ledger, PlanAction, PlannedChange,
};

/// Drain time between moving traffic and stopping the old container in a
/// start-first update, unless the component sets `stop_grace`.
pub const DEFAULT_STOP_GRACE: Duration = Duration::from_secs(2);

/// Options for [`Engine::apply`].
#[derive(Debug, Clone, Copy, Default)]
pub struct ApplyOptions {
    /// Stop and remove containers that are no longer declared.
    pub prune: bool,
}

/// Result of [`Engine::apply`].
#[derive(Debug, Clone)]
pub struct ApplyOutcome {
    /// The plan that was executed.
    pub plan: DeployPlan,
    /// Declared components, now running, in dependency order.
    pub deployed: Vec<DeployedComponent>,
    /// Names of containers removed by `prune`.
    pub removed: Vec<String>,
}

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

    /// Computes the declarative plan for a `.ctst` file without changing
    /// anything on the host.
    ///
    /// # Errors
    ///
    /// Returns an error if the composition is invalid or state cannot be read.
    pub fn plan(&self, ctst_path: &Path) -> Result<DeployPlan> {
        let desired = load_desired(ctst_path, self.offline, self.backend.supports_handoff())?;
        // Keep planning read-only: a native project without a state directory
        // has no containers, and listing would create the lock file.
        let has_local_state = self.state_file.parent().is_some_and(Path::exists)
            || self.backend.as_any().is::<crate::backend::vm::VMBackend>();
        let live = if has_local_state {
            self.backend.list()?
        } else {
            Vec::new()
        };
        self.plan_for(&desired, &live)
    }

    /// Applies a `.ctst` file declaratively: unchanged running components
    /// are kept, stopped ones started, changed ones replaced one at a time in
    /// dependency order, and new ones created. With `prune`, containers no
    /// longer declared are stopped and removed.
    ///
    /// # Errors
    ///
    /// Returns an error if planning or any backend operation fails. Changes
    /// applied before the failure are kept and recorded in the ledger.
    pub fn apply(&self, ctst_path: &Path, options: ApplyOptions) -> Result<ApplyOutcome> {
        let project_dir = containust_common::constants::project_dir(ctst_path);
        for subdir in ["logs", "state"] {
            let path = project_dir.join(subdir);
            std::fs::create_dir_all(&path)
                .map_err(|source| ContainustError::Io { path, source })?;
        }
        tracing::info!(project_dir = %project_dir.display(), "project directory");

        let desired = load_desired(ctst_path, self.offline, self.backend.supports_handoff())?;
        let _apply_lock = ApplyLock::acquire(&self.state_file)?;
        self.recover_interrupted_updates(&desired)?;
        let live = self.backend.list()?;
        let plan = self.plan_for(&desired, &live)?;
        let ledger_path = Ledger::path_for(&self.state_file);
        let mut run = ApplyRun {
            engine: self,
            prune: options.prune,
            ledger: Ledger::load(&ledger_path)?,
            ledger_path,
            live: live.iter().map(|info| (info.name.as_str(), info)).collect(),
            desired: desired
                .iter()
                .map(|spec| (spec.component.name.as_str(), spec))
                .collect(),
            deployed: Vec::with_capacity(desired.len()),
            removed: Vec::new(),
        };
        for change in &plan.changes {
            run.apply_change(change)?;
        }
        Ok(ApplyOutcome {
            deployed: run.deployed,
            removed: run.removed,
            plan,
        })
    }

    fn plan_for(&self, desired: &[DesiredSpec], live: &[ContainerInfo]) -> Result<DeployPlan> {
        let ledger = Ledger::load(&Ledger::path_for(&self.state_file))?;
        let inputs: Vec<DesiredComponent<'_>> = desired
            .iter()
            .map(|spec| DesiredComponent {
                name: &spec.component.name,
                fingerprint: spec.fingerprint.clone(),
                upstreams: spec.upstreams.iter().map(String::as_str).collect(),
                strategy: spec.strategy,
                note: spec.strategy_note.clone(),
            })
            .collect();
        let mut plan = crate::plan::compute(&inputs, live, &ledger);
        plan.warnings = connect_warnings(desired);
        Ok(plan)
    }

    /// Settles staged `<name>~next` containers left by an interrupted
    /// start-first update. One that never received the ports is removed
    /// (the original still serves); one that already serves them is
    /// promoted: the original is retired and the staged one takes its name.
    fn recover_interrupted_updates(&self, desired: &[DesiredSpec]) -> Result<()> {
        let live = self.backend.list()?;
        let ledger_path = Ledger::path_for(&self.state_file);
        let mut ledger = Ledger::load(&ledger_path)?;
        let before = ledger.clone();
        for info in &live {
            let Some(owner) = crate::plan::staged_owner(&info.name)
                .filter(|owner| desired.iter().any(|spec| spec.component.name == *owner))
            else {
                continue;
            };
            let recorded = ledger.components.remove(&info.name);
            if self.backend.is_staged(&info.id)? {
                eprintln!(
                    "  Removing '{}' left over from an interrupted update...",
                    info.name
                );
                self.retire(&info.id, info.state == "running")?;
            } else {
                let original = live.iter().find(|other| other.name == owner);
                let applied = self.promote_staged(info, original, recorded)?;
                let _ = ledger.components.insert(owner.to_string(), applied);
            }
        }
        if ledger != before {
            ledger.save(&ledger_path)?;
        }
        Ok(())
    }

    /// Completes an update whose staged container already serves traffic.
    fn promote_staged(
        &self,
        staged: &ContainerInfo,
        original: Option<&ContainerInfo>,
        recorded: Option<AppliedComponent>,
    ) -> Result<AppliedComponent> {
        let owner = crate::plan::staged_owner(&staged.name).unwrap_or(&staged.name);
        eprintln!("  Completing the interrupted update of '{owner}'...");
        if let Some(original) = original {
            self.retire(&original.id, original.state == "running")?;
        }
        self.backend.rename(&staged.id, owner)?;
        // Without a record of what the staged spec was, an empty
        // fingerprint makes the next plan replace it (from a serving state).
        let fingerprint = recorded
            .filter(|applied| applied.container_id == staged.id)
            .map(|applied| applied.fingerprint)
            .unwrap_or_default();
        Ok(AppliedComponent {
            fingerprint,
            container_id: staged.id.clone(),
        })
    }

    /// Stops (when running) and removes an existing container.
    fn retire(&self, id: &ContainerId, running: bool) -> Result<()> {
        if running {
            self.stop_with_force(id, false)?;
        }
        self.backend.remove(id)
    }

    fn deploy_inner(&self, ctst_path: &Path) -> Result<Vec<DeployedComponent>> {
        self.apply(ctst_path, ApplyOptions::default())
            .map(|outcome| outcome.deployed)
    }

    /// Creates and starts one planned component.
    fn deploy_component(&self, spec: &DesiredSpec) -> Result<DeployedComponent> {
        self.deploy_config(spec, spec.config.clone())
    }

    /// Creates and starts a container for `spec` from `config`, which may
    /// differ in name and publishing (start-first staging).
    fn deploy_config(
        &self,
        spec: &DesiredSpec,
        mut config: ContainerConfig,
    ) -> Result<DeployedComponent> {
        let comp = &spec.component;
        config.image = resolve_deploy_image(self.data_dir(), self.offline, comp)?;

        eprintln!("  Creating container '{}'...", config.name);
        let id = self.backend.create(&config)?;
        tracing::info!(id = %id, name = %config.name, "container created");

        eprintln!("  Starting container '{}'...", config.name);
        let pid = match self.backend.start(&id) {
            Ok(pid) => pid,
            Err(error) => {
                // A staged replacement must not linger next to the original.
                if config.defer_publish {
                    let _ = self.backend.remove(&id);
                }
                return Err(error);
            }
        };
        tracing::info!(id = %id, pid, name = %config.name, "container started");

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

/// Project-wide exclusive lock held for a whole [`Engine::apply`], so two
/// concurrent `ctst run`s cannot interleave (e.g. one discarding the
/// other's staged start-first container).
struct ApplyLock {
    file: std::fs::File,
}

impl ApplyLock {
    fn acquire(state_file: &Path) -> Result<Self> {
        use fs2::FileExt;

        let path = state_file.with_file_name("apply.lock");
        let io_err = |source| ContainustError::Io {
            path: path.clone(),
            source,
        };
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(io_err)?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(io_err)?;
        file.try_lock_exclusive()
            .map_err(|_| ContainustError::Config {
                message: format!(
                    "another `ctst run` is applying this project (lock: {}); \
                 wait for it to finish",
                    path.display()
                ),
            })?;
        Ok(Self { file })
    }
}

impl Drop for ApplyLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.file);
    }
}

/// Mutable bookkeeping for one [`Engine::apply`] pass.
struct ApplyRun<'a> {
    engine: &'a Engine,
    prune: bool,
    ledger: Ledger,
    ledger_path: PathBuf,
    live: HashMap<&'a str, &'a ContainerInfo>,
    desired: HashMap<&'a str, &'a DesiredSpec>,
    deployed: Vec<DeployedComponent>,
    removed: Vec<String>,
}

impl ApplyRun<'_> {
    fn apply_change(&mut self, change: &PlannedChange) -> Result<()> {
        match change.action {
            PlanAction::Remove if self.prune => self.remove(change),
            PlanAction::Remove => Ok(()),
            PlanAction::Keep | PlanAction::Start => self.keep_or_start(change),
            PlanAction::Create | PlanAction::Replace => self.create_or_replace(change),
        }
    }

    fn is_running(&self, name: &str) -> bool {
        self.live
            .get(name)
            .is_some_and(|info| info.state == "running")
    }

    fn forget(&mut self, name: &str) -> Result<()> {
        let _ = self.ledger.components.remove(name);
        self.ledger.save(&self.ledger_path)
    }

    fn remove(&mut self, change: &PlannedChange) -> Result<()> {
        if let Some(id) = &change.container_id {
            eprintln!("  Removing container '{}'...", change.name);
            self.engine.retire(id, self.is_running(&change.name))?;
        }
        self.forget(&change.name)?;
        self.removed.push(change.name.clone());
        Ok(())
    }

    fn keep_or_start(&mut self, change: &PlannedChange) -> Result<()> {
        let (Some(spec), Some(id)) = (
            self.desired.get(change.name.as_str()).copied(),
            change.container_id.clone(),
        ) else {
            return Ok(());
        };
        let pid = if change.action == PlanAction::Start {
            eprintln!("  Starting container '{}'...", change.name);
            Some(self.engine.backend.start(&id)?)
        } else {
            eprintln!("  Keeping container '{}' (unchanged)", change.name);
            self.live
                .get(change.name.as_str())
                .and_then(|info| info.pid)
        };
        self.deployed.push(DeployedComponent {
            id,
            name: change.name.clone(),
            port: spec.component.port,
            pid,
        });
        Ok(())
    }

    fn create_or_replace(&mut self, change: &PlannedChange) -> Result<()> {
        let Some(spec) = self.desired.get(change.name.as_str()).copied() else {
            return Ok(());
        };
        if let (Some(UpdateStrategy::StartFirst), Some(old)) =
            (change.strategy, change.container_id.as_ref())
        {
            return self.start_first_replace(change, spec, old);
        }
        if let Some(id) = &change.container_id {
            eprintln!(
                "  Replacing container '{}' ({})...",
                change.name,
                change.reason.as_deref().unwrap_or("spec changed")
            );
            self.engine.retire(id, self.is_running(&change.name))?;
            self.forget(&change.name)?;
        }
        let component = self.engine.deploy_component(spec)?;
        let _ = self.ledger.components.insert(
            change.name.clone(),
            AppliedComponent {
                fingerprint: spec.fingerprint.clone(),
                container_id: component.id.clone(),
            },
        );
        self.ledger.save(&self.ledger_path)?;
        self.deployed.push(component);
        Ok(())
    }

    /// Replaces a running component without dropping its published ports:
    /// stage the new container, gate on its healthcheck, hand the ports
    /// over, drain, retire the old one, and take over its name.
    fn start_first_replace(
        &mut self,
        change: &PlannedChange,
        spec: &DesiredSpec,
        old: &ContainerId,
    ) -> Result<()> {
        eprintln!(
            "  Replacing container '{}' start-first ({})...",
            change.name,
            change.reason.as_deref().unwrap_or("spec changed")
        );
        let staged = self.stage_and_switch(&change.name, spec, old)?;
        // From here the staged container serves; record it so an
        // interrupted run is completed (not undone) by the next one.
        let staged_name = crate::plan::staged_name(&change.name);
        self.record_applied(&staged_name, &spec.fingerprint, &staged.id)?;
        eprintln!(
            "  Traffic moved to the new '{}'; draining the old one for {}s...",
            change.name,
            spec.stop_grace.as_secs_f32()
        );
        std::thread::sleep(spec.stop_grace);
        self.engine.retire(old, true)?;
        self.engine.backend.rename(&staged.id, &change.name)?;
        let _ = self.ledger.components.remove(&staged_name);
        self.record_applied(&change.name, &spec.fingerprint, &staged.id)?;
        self.deployed.push(staged);
        Ok(())
    }

    /// Starts `<name>~next` unpublished, waits until it is healthy, and
    /// moves the ports to it. On failure it is removed and `old` keeps
    /// (or gets back) the ports.
    fn stage_and_switch(
        &self,
        name: &str,
        spec: &DesiredSpec,
        old: &ContainerId,
    ) -> Result<DeployedComponent> {
        let backend = &self.engine.backend;
        let mut config = spec.config.clone();
        config.name = crate::plan::staged_name(name);
        config.defer_publish = true;
        let staged = self.engine.deploy_config(spec, config)?;
        let gate = spec.config.healthcheck.as_ref().map_or(Ok(()), |health| {
            eprintln!("  Waiting for the new '{name}' to become healthy...");
            crate::supervise::wait_healthy(backend.as_ref(), &staged.id, health)
        });
        let switched = gate
            .map_err(|message| ContainustError::Config {
                message: format!(
                    "new '{name}' never became healthy; the old container keeps serving: {message}"
                ),
            })
            .and_then(|()| backend.handoff(old, &staged.id));
        if let Err(error) = switched {
            let _ = self.engine.retire(&staged.id, true);
            return Err(error);
        }
        Ok(staged)
    }

    /// Records the applied spec of `name` and persists the ledger.
    fn record_applied(&mut self, name: &str, fingerprint: &str, id: &ContainerId) -> Result<()> {
        let _ = self.ledger.components.insert(
            name.to_string(),
            AppliedComponent {
                fingerprint: fingerprint.to_string(),
                container_id: id.clone(),
            },
        );
        self.ledger.save(&self.ledger_path)
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

/// A declared component with its fully-built (pre-image-resolution) config.
struct DesiredSpec {
    component: containust_compose::parser::ast::ComponentDecl,
    config: ContainerConfig,
    fingerprint: String,
    upstreams: Vec<String>,
    /// Effective update strategy (after fallbacks).
    strategy: UpdateStrategy,
    /// Why a declared `start-first` fell back to `recreate`.
    strategy_note: Option<String>,
    /// Drain time before the old container is stopped (start-first).
    stop_grace: Duration,
}

/// Parses and validates a composition into desired specs in deploy order.
///
/// Every component is validated up front so an invalid file fails before
/// any container is touched.
fn load_desired(
    ctst_path: &Path,
    offline: bool,
    supports_handoff: bool,
) -> Result<Vec<DesiredSpec>> {
    let composition = read_composition(ctst_path, offline)?;
    let order = resolve_deploy_order(&composition)?;
    let resolved = containust_compose::resolver::resolve_connections(&composition)?;
    let mut components: HashMap<&str, &containust_compose::parser::ast::ComponentDecl> =
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
    let mut upstreams = upstreams_by_component(&composition);

    let mut desired = Vec::with_capacity(order.len());
    for name in &order {
        let component =
            components
                .remove(name.as_str())
                .ok_or_else(|| ContainustError::NotFound {
                    kind: "component",
                    id: name.clone(),
                })?;
        validate_runtime_component(component)?;
        let mappings = published_port_mappings(component, &composition.exposes)?;
        let config = build_deploy_config(
            component,
            resolved_by_name.get(name.as_str()).copied(),
            component.image.clone().unwrap_or_default(),
            mappings,
        )?;
        let update = resolve_update(component, &config, supports_handoff)?;
        desired.push(DesiredSpec {
            fingerprint: crate::plan::fingerprint(&config),
            upstreams: upstreams.remove(name.as_str()).unwrap_or_default(),
            component: component.clone(),
            config,
            strategy: update.strategy,
            strategy_note: update.note,
            stop_grace: update.stop_grace,
        });
    }
    ensure_unique_host_ports(&desired)?;
    Ok(desired)
}

/// Warns about `CONNECT`s whose target cannot be reached by name: peers
/// resolve each other only inside one shared network (`bridge` or a
/// custom name), where `/etc/hosts` maps every member to loopback.
fn connect_warnings(desired: &[DesiredSpec]) -> Vec<String> {
    let network_of: HashMap<&str, &str> = desired
        .iter()
        .map(|spec| (spec.component.name.as_str(), spec.config.network.as_str()))
        .collect();
    let mut warnings = Vec::new();
    for spec in desired {
        let from_net = spec.config.network.as_str();
        for upstream in &spec.upstreams {
            let to_net = network_of
                .get(upstream.as_str())
                .copied()
                .unwrap_or_default();
            let shared = crate::network::NetworkMode::parse(Some(from_net))
                .shared_name()
                .is_some();
            if shared && from_net == to_net {
                continue;
            }
            let why = if from_net == to_net {
                format!("neither is on a shared network ('{from_net}')")
            } else {
                format!("they are on different networks ('{from_net}' vs '{to_net}')")
            };
            warnings.push(format!(
                "'{}' CONNECTs to '{upstream}' but {why}, so '{upstream}' will not \
                 resolve; set the same `network = \"<name>\"` on both",
                spec.component.name
            ));
        }
    }
    warnings
}

/// Rejects two components publishing the same host port: the second
/// publish would fail at start, after the first one was already deployed.
fn ensure_unique_host_ports(desired: &[DesiredSpec]) -> Result<()> {
    let mut owners: HashMap<u16, &str> = HashMap::new();
    for spec in desired {
        for mapping in &spec.config.port_mappings {
            if let Some(other) = owners.insert(mapping.host, &spec.component.name) {
                return Err(ContainustError::Config {
                    message: format!(
                        "host port {} is published by both '{other}' and '{}'",
                        mapping.host, spec.component.name
                    ),
                });
            }
        }
    }
    Ok(())
}

/// Reads a composition (resolving IMPORT / FROM), enforcing offline policy.
fn read_composition(
    ctst_path: &Path,
    offline: bool,
) -> Result<containust_compose::parser::ast::CompositionFile> {
    let composition = containust_compose::template::load_file(ctst_path)?;
    if offline {
        containust_compose::validate_offline(&composition)?;
    }
    Ok(composition)
}

/// A component's resolved `update` / `stop_grace` settings.
struct UpdatePolicy {
    strategy: UpdateStrategy,
    note: Option<String>,
    stop_grace: Duration,
}

/// Resolves the declared update strategy, falling back to `recreate` (with
/// the reason) where two copies of the component cannot run side by side.
///
/// Neither setting is part of the spec fingerprint: changing only how a
/// component is updated does not replace it.
fn resolve_update(
    comp: &containust_compose::parser::ast::ComponentDecl,
    config: &ContainerConfig,
    supports_handoff: bool,
) -> Result<UpdatePolicy> {
    let invalid = |message: String| ContainustError::Config {
        message: format!("component '{}': {message}", comp.name),
    };
    let strategy = comp
        .update
        .as_deref()
        .map(UpdateStrategy::parse)
        .transpose()
        .map_err(invalid)?
        .unwrap_or_default();
    let stop_grace = match comp.stop_grace.as_deref() {
        Some(text) => parse_duration_secs(text)
            .map(Duration::from_secs)
            .ok_or_else(|| {
                invalid(format!(
                    "invalid stop_grace '{text}' (expected e.g. \"5s\"; at most 24h)"
                ))
            })?,
        None => DEFAULT_STOP_GRACE,
    };
    if strategy == UpdateStrategy::StartFirst && config.healthcheck.is_none() {
        return Err(invalid(
            "update = \"start-first\" needs a healthcheck to decide when the new \
             container is ready"
                .into(),
        ));
    }
    let note = (strategy == UpdateStrategy::StartFirst)
        .then(|| start_first_blocker(config, supports_handoff))
        .flatten();
    Ok(UpdatePolicy {
        strategy: if note.is_some() {
            UpdateStrategy::Recreate
        } else {
            strategy
        },
        note,
        stop_grace,
    })
}

/// Why a start-first update cannot run for this config, if it cannot.
fn start_first_blocker(config: &ContainerConfig, supports_handoff: bool) -> Option<String> {
    let network = crate::network::NetworkMode::parse(Some(config.network.as_str()));
    if !supports_handoff {
        Some("start-first is not supported by this backend yet; using recreate".into())
    } else if network.is_host() {
        Some(
            "host network: old and new container cannot bind the same port; \
             set network = \"none\" to publish through the forwarder"
                .into(),
        )
    } else {
        network.shared_name().map(|name| {
            format!(
                "shared network '{name}': old and new container would listen in the \
                 same network namespace; using recreate"
            )
        })
    }
}

/// Maps each component to the components it `CONNECT`s to.
fn upstreams_by_component(
    composition: &containust_compose::parser::ast::CompositionFile,
) -> HashMap<&str, Vec<String>> {
    let mut upstreams: HashMap<&str, Vec<String>> = HashMap::new();
    for conn in &composition.connections {
        upstreams
            .entry(conn.from.as_str())
            .or_default()
            .push(conn.to.clone());
    }
    upstreams
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
    if let Some(mode) = component.network.as_deref() {
        let trimmed = mode.trim();
        if trimmed.is_empty() {
            return Err(ContainustError::Config {
                message: format!("component '{}' has an empty network name", component.name),
            });
        }
    }
    Ok(())
}

/// Merges a component's `ports` list with `EXPOSE` statements targeting it.
///
/// Supports host:container remapping. Duplicate host ports fail closed.
///
/// # Errors
///
/// Returns an error when host ports collide.
fn build_deploy_config(
    comp: &containust_compose::parser::ast::ComponentDecl,
    resolved_comp: Option<&containust_compose::resolver::ResolvedComponent>,
    image: String,
    port_mappings: Vec<containust_common::types::PortMapping>,
) -> Result<ContainerConfig> {
    let memory_bytes = parse_optional_memory(comp.memory.as_deref())?;
    let cpu_shares = parse_optional_cpu(comp.cpu.as_deref())?;
    let restart = parse_restart_policy(comp)?;
    let healthcheck = comp
        .healthcheck
        .as_ref()
        .map(|decl| parse_healthcheck_spec(&comp.name, decl))
        .transpose()?;
    let network = resolve_deploy_network(comp.network.as_deref(), &port_mappings);
    let namespaces = namespaces_for_network(&network);
    let network_name = match &network {
        crate::network::NetworkMode::Host => "host".into(),
        crate::network::NetworkMode::None => "none".into(),
        crate::network::NetworkMode::Shared(name) => name.clone(),
    };
    Ok(ContainerConfig {
        name: comp.name.clone(),
        image,
        command: effective_command(comp),
        env: resolved_comp.map_or_else(Vec::new, |r| r.env.clone()),
        memory_bytes,
        cpu_shares,
        readonly_rootfs: comp.readonly.unwrap_or(true),
        volumes: component_volumes(comp),
        port: comp.port,
        ports: port_mappings.iter().map(|m| m.container).collect(),
        port_mappings,
        network: network_name,
        restart,
        healthcheck,
        namespaces,
        defer_publish: false,
    })
}

fn resolve_deploy_network(
    declared: Option<&str>,
    port_mappings: &[containust_common::types::PortMapping],
) -> crate::network::NetworkMode {
    let has_remap = port_mappings.iter().any(|m| m.is_remap());
    // Back-compat: identity-only publishes with no explicit network → host.
    let network = if declared.is_none() && !port_mappings.is_empty() && !has_remap {
        crate::network::NetworkMode::Host
    } else {
        crate::network::NetworkMode::parse(declared)
    };
    // Remap needs a non-host netns for the userspace forwarder; keep it
    // private so per-container userns remains compatible (shared netns is
    // owned by the init userns and cannot be joined after CLONE_NEWUSER).
    if has_remap && network.is_host() {
        return crate::network::NetworkMode::None;
    }
    network
}

/// Namespace policy for a deploy network mode.
///
/// Shared project netns files are created in the init user namespace, so
/// containers that join them must not enter a private userns (setns would
/// fail closed after the uid-map handshake).
fn namespaces_for_network(
    network: &crate::network::NetworkMode,
) -> containust_core::namespace::NamespaceConfig {
    let mut namespaces = containust_core::namespace::NamespaceConfig::default().with_user_and_pid();
    namespaces.network = !network.is_host();
    if network.shared_name().is_some() {
        namespaces.user = false;
    }
    namespaces
}

fn published_port_mappings(
    comp: &containust_compose::parser::ast::ComponentDecl,
    exposes: &[containust_compose::parser::ast::ExposeDecl],
) -> Result<Vec<containust_common::types::PortMapping>> {
    use containust_common::types::PortMapping;
    use std::collections::HashSet;

    let declared: HashSet<u16> = comp.port.iter().chain(comp.ports.iter()).copied().collect();
    let mut mappings: Vec<PortMapping> = comp
        .ports
        .iter()
        .copied()
        .map(PortMapping::identity)
        .collect();
    let mut host_ports: HashSet<u16> = mappings.iter().map(|m| m.host).collect();

    for expose in exposes {
        if !declared.contains(&expose.container_port) {
            continue;
        }
        let mapping = PortMapping {
            host: expose.host_port,
            container: expose.container_port,
        };
        // `ports = [8080]` plus `EXPOSE 8080` publishes the same mapping twice.
        if mappings.contains(&mapping) {
            continue;
        }
        if !host_ports.insert(mapping.host) {
            return Err(ContainustError::Config {
                message: format!(
                    "duplicate host port {} while publishing component '{}'",
                    mapping.host, comp.name
                ),
            });
        }
        mappings.retain(|m| m.container != mapping.container || m.host == mapping.host);
        if !mappings
            .iter()
            .any(|m| m.host == mapping.host && m.container == mapping.container)
        {
            mappings.push(mapping);
        }
    }
    Ok(mappings)
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
             (expected e.g. \"30s\", \"1m\", \"1h\"; at most 24h)"
        ),
    })
}

/// Longest accepted duration (healthcheck timings, `stop_grace`): one day.
const MAX_DURATION_SECS: u64 = 24 * 3600;

/// Parses `"30s"`, `"5m"`, `"1h"`, or a plain seconds integer, up to
/// [`MAX_DURATION_SECS`].
fn parse_duration_secs(text: &str) -> Option<u64> {
    const UNITS: [(char, u64); 3] = [('h', 3600), ('m', 60), ('s', 1)];
    let text = text.trim();
    let (digits, multiplier) = UNITS
        .iter()
        .find_map(|&(suffix, mult)| text.strip_suffix(suffix).map(|digits| (digits, mult)))
        .unwrap_or((text, 1));
    digits
        .trim()
        .parse::<u64>()
        .ok()
        .and_then(|n| n.checked_mul(multiplier))
        .filter(|&secs| secs <= MAX_DURATION_SECS)
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

    /// Backend that keeps containers in memory so apply can be re-run.
    #[derive(Default)]
    struct MemBackend {
        containers: Mutex<Vec<ContainerInfo>>,
        log: Mutex<Vec<String>>,
        next: std::sync::atomic::AtomicU32,
        unhealthy: AtomicBool,
        staged: Mutex<std::collections::HashSet<ContainerId>>,
    }

    impl MemBackend {
        fn record(&self, entry: String) {
            self.log.lock().expect("log").push(entry);
        }

        fn set_state(&self, id: &ContainerId, state: &str) {
            self.containers
                .lock()
                .expect("containers")
                .iter_mut()
                .filter(|info| info.id == *id)
                .for_each(|info| info.state = state.into());
        }
    }

    impl ContainerBackend for Arc<MemBackend> {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn create(&self, config: &ContainerConfig) -> Result<ContainerId> {
            let n = self.next.fetch_add(1, Ordering::Relaxed);
            let id = ContainerId::new(format!("{}-{n}", config.name));
            self.containers
                .lock()
                .expect("containers")
                .push(ContainerInfo {
                    id: id.clone(),
                    name: config.name.clone(),
                    state: "created".into(),
                    pid: None,
                    image: config.image.clone(),
                    created_at: String::new(),
                });
            if config.defer_publish {
                let _ = self.staged.lock().expect("staged").insert(id.clone());
            }
            self.record(format!("create {}", config.name));
            Ok(id)
        }

        fn start(&self, id: &ContainerId) -> Result<u32> {
            self.set_state(id, "running");
            self.record(format!("start {id}"));
            Ok(7)
        }

        fn stop(&self, id: &ContainerId) -> Result<()> {
            self.set_state(id, "stopped");
            self.record(format!("stop {id}"));
            Ok(())
        }

        fn exec(&self, id: &ContainerId, _cmd: &[String]) -> Result<ExecOutput> {
            self.record(format!("probe {id}"));
            Ok(ExecOutput {
                stdout: String::new(),
                stderr: "connection refused".into(),
                exit_code: i32::from(self.unhealthy.load(Ordering::Acquire)),
            })
        }

        fn supports_handoff(&self) -> bool {
            true
        }

        fn handoff(&self, from: &ContainerId, to: &ContainerId) -> Result<()> {
            let _ = self.staged.lock().expect("staged").remove(to);
            self.record(format!("handoff {from} -> {to}"));
            Ok(())
        }

        fn is_staged(&self, id: &ContainerId) -> Result<bool> {
            Ok(self.staged.lock().expect("staged").contains(id))
        }

        fn rename(&self, id: &ContainerId, name: &str) -> Result<()> {
            self.containers
                .lock()
                .expect("containers")
                .iter_mut()
                .filter(|info| info.id == *id)
                .for_each(|info| info.name = name.into());
            self.record(format!("rename {id} {name}"));
            Ok(())
        }

        fn remove(&self, id: &ContainerId) -> Result<()> {
            self.containers
                .lock()
                .expect("containers")
                .retain(|info| info.id != *id);
            self.record(format!("remove {id}"));
            Ok(())
        }

        fn logs(&self, _id: &ContainerId) -> Result<String> {
            Ok(String::new())
        }

        fn list(&self) -> Result<Vec<ContainerInfo>> {
            Ok(self.containers.lock().expect("containers").clone())
        }

        fn is_available(&self) -> bool {
            true
        }
    }

    fn mem_engine(backend: &Arc<MemBackend>, data_dir: PathBuf) -> Engine {
        let options = EngineOptions {
            state_file: data_dir.join("state").join("state.json"),
            data_dir,
            offline: false,
        };
        Engine::with_backend(options, Box::new(Arc::clone(backend)))
    }

    fn take_log(backend: &MemBackend) -> Vec<String> {
        std::mem::take(&mut *backend.log.lock().expect("log"))
    }

    const STACK: &str = r#"COMPONENT db {
    image = "file:///db"
    port = 5432
}
COMPONENT api {
    image = "file:///api"
    port = 8080
}
CONNECT api -> db
"#;

    #[test]
    fn apply_is_idempotent_for_unchanged_stack() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("stack.ctst");
        std::fs::write(&file, STACK).expect("write");
        let backend = Arc::new(MemBackend::default());
        let engine = mem_engine(&backend, dir.path().join("data"));

        let first = engine.apply(&file, ApplyOptions::default()).expect("apply");
        assert_eq!(first.plan.count(PlanAction::Create), 2);
        assert_eq!(
            take_log(&backend),
            vec!["create db", "start db-0", "create api", "start api-1"]
        );

        let plan = engine.plan(&file).expect("plan");
        assert!(plan.is_noop(), "{plan:?}");
        let second = engine
            .apply(&file, ApplyOptions::default())
            .expect("re-apply");
        assert_eq!(second.deployed.len(), 2);
        assert_eq!(take_log(&backend), Vec::<String>::new());
    }

    #[test]
    fn apply_replaces_changed_component_and_its_dependents() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("stack.ctst");
        std::fs::write(&file, STACK).expect("write");
        let backend = Arc::new(MemBackend::default());
        let engine = mem_engine(&backend, dir.path().join("data"));
        let _ = engine.apply(&file, ApplyOptions::default()).expect("apply");
        let _ = take_log(&backend);

        std::fs::write(&file, STACK.replace("file:///db", "file:///db2")).expect("rewrite");
        let plan = engine.plan(&file).expect("plan");
        assert_eq!(plan.count(PlanAction::Replace), 2);
        let _ = engine.apply(&file, ApplyOptions::default()).expect("apply");
        assert_eq!(
            take_log(&backend),
            vec![
                "stop db-0",
                "remove db-0",
                "create db",
                "start db-2",
                "stop api-1",
                "remove api-1",
                "create api",
                "start api-3",
            ]
        );
        assert!(engine.plan(&file).expect("plan").is_noop());
    }

    #[test]
    fn apply_starts_stopped_and_prunes_undeclared() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("stack.ctst");
        std::fs::write(&file, STACK).expect("write");
        let backend = Arc::new(MemBackend::default());
        let engine = mem_engine(&backend, dir.path().join("data"));
        let _ = engine.apply(&file, ApplyOptions::default()).expect("apply");
        backend.set_state(&ContainerId::new("db-0"), "stopped");
        let _ = take_log(&backend);

        let db_only = STACK.split("COMPONENT api").next().expect("db block");
        std::fs::write(&file, db_only).expect("rewrite");
        let kept = engine.apply(&file, ApplyOptions::default()).expect("apply");
        assert_eq!(kept.removed, Vec::<String>::new());
        assert_eq!(take_log(&backend), vec!["start db-0"]);
        assert_eq!(engine.list().expect("list").len(), 2);

        let pruned = engine
            .apply(&file, ApplyOptions { prune: true })
            .expect("prune");
        assert_eq!(pruned.removed, vec!["api"]);
        assert_eq!(take_log(&backend), vec!["stop api-1", "remove api-1"]);
        assert_eq!(engine.list().expect("list").len(), 1);
    }

    const ROLLING: &str = r#"COMPONENT web {
    image = "file:///web/v1"
    network = "none"
    ports = [8080]
    update = "start-first"
    stop_grace = "0s"
    healthcheck = {
        command = ["true"]
        interval = "0s"
        retries = 2
    }
}
"#;

    fn deployed_rolling(backend: &Arc<MemBackend>) -> (tempfile::TempDir, PathBuf, Engine) {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("stack.ctst");
        std::fs::write(&file, ROLLING).expect("write");
        let engine = mem_engine(backend, dir.path().join("data"));
        let _ = engine.apply(&file, ApplyOptions::default()).expect("apply");
        let _ = take_log(backend);
        std::fs::write(&file, ROLLING.replace("web/v1", "web/v2")).expect("rewrite");
        (dir, file, engine)
    }

    #[test]
    fn start_first_stages_gates_hands_off_then_retires_old() {
        let backend = Arc::new(MemBackend::default());
        let (_dir, file, engine) = deployed_rolling(&backend);

        let plan = engine.plan(&file).expect("plan");
        assert_eq!(plan.changes[0].strategy, Some(UpdateStrategy::StartFirst));
        let outcome = engine.apply(&file, ApplyOptions::default()).expect("apply");

        assert_eq!(
            take_log(&backend),
            vec![
                "create web~next",
                "start web~next-1",
                "probe web~next-1",
                "handoff web-0 -> web~next-1",
                "stop web-0",
                "remove web-0",
                "rename web~next-1 web",
            ]
        );
        assert_eq!(outcome.deployed[0].name, "web");
        let names: Vec<_> = engine
            .list()
            .expect("list")
            .into_iter()
            .map(|info| info.name)
            .collect();
        assert_eq!(names, ["web"]);
        assert!(engine.plan(&file).expect("plan").is_noop());
    }

    #[test]
    fn unhealthy_replacement_is_discarded_and_old_keeps_serving() {
        let backend = Arc::new(MemBackend::default());
        let (_dir, file, engine) = deployed_rolling(&backend);
        backend.unhealthy.store(true, Ordering::Release);

        let error = engine
            .apply(&file, ApplyOptions::default())
            .expect_err("gate must fail");
        assert!(
            error.to_string().contains("never became healthy"),
            "{error}"
        );

        let log = take_log(&backend);
        assert_eq!(log.iter().filter(|e| e.starts_with("probe")).count(), 2);
        assert!(!log.iter().any(|e| e.starts_with("handoff")));
        assert_eq!(
            &log[log.len() - 2..],
            ["stop web~next-1", "remove web~next-1"]
        );
        let live = engine.list().expect("list");
        assert_eq!(live.len(), 1);
        assert_eq!(
            (live[0].name.as_str(), live[0].state.as_str()),
            ("web", "running")
        );
        // The ledger still records v1, so the change is retried next time.
        assert_eq!(
            engine.plan(&file).expect("plan").count(PlanAction::Replace),
            1
        );
    }

    #[test]
    fn leftover_staged_container_is_removed_before_apply() {
        let backend = Arc::new(MemBackend::default());
        let (_dir, file, engine) = deployed_rolling(&backend);
        let stray = ContainerConfig {
            name: "web~next".into(),
            image: "file:///web/v0".into(),
            command: Vec::new(),
            env: Vec::new(),
            memory_bytes: None,
            cpu_shares: None,
            readonly_rootfs: true,
            volumes: Vec::new(),
            port: None,
            ports: Vec::new(),
            port_mappings: Vec::new(),
            network: "none".into(),
            restart: containust_common::types::RestartPolicy::Never,
            healthcheck: None,
            namespaces: containust_core::namespace::NamespaceConfig::default(),
            defer_publish: true,
        };
        let id = backend.create(&stray).expect("stray");
        let _ = backend.start(&id).expect("start stray");
        let _ = take_log(&backend);

        let plan = engine.plan(&file).expect("plan");
        assert!(plan.changes.iter().any(|c| {
            c.name == "web~next"
                && c.reason
                    .as_deref()
                    .is_some_and(|r| r.contains("interrupted"))
        }));
        let _ = engine.apply(&file, ApplyOptions::default()).expect("apply");
        let log = take_log(&backend);
        assert_eq!(&log[..2], ["stop web~next-1", "remove web~next-1"]);
        assert_eq!(engine.list().expect("list").len(), 1);
    }

    #[test]
    fn start_first_requires_healthcheck_and_falls_back_on_host_network() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("stack.ctst");
        let backend = Arc::new(MemBackend::default());
        let engine = mem_engine(&backend, dir.path().join("data"));

        let no_probe = "COMPONENT web {\n image = \"file:///w\"\n update = \"start-first\"\n}\n";
        std::fs::write(&file, no_probe).expect("write");
        let error = engine.plan(&file).expect_err("needs healthcheck");
        assert!(error.to_string().contains("needs a healthcheck"), "{error}");

        let host = ROLLING.replace("    network = \"none\"\n", "");
        std::fs::write(&file, &host).expect("write");
        let _ = engine.apply(&file, ApplyOptions::default()).expect("apply");
        std::fs::write(&file, host.replace("web/v1", "web/v2")).expect("rewrite");
        let plan = engine.plan(&file).expect("plan");
        assert_eq!(plan.changes[0].strategy, Some(UpdateStrategy::Recreate));
        assert!(
            plan.changes[0]
                .note
                .as_deref()
                .is_some_and(|n| n.contains("host network"))
        );
    }

    #[test]
    fn changing_only_update_settings_is_not_a_replace() {
        let backend = Arc::new(MemBackend::default());
        let (_dir, file, engine) = deployed_rolling(&backend);
        std::fs::write(
            &file,
            ROLLING.replace("stop_grace = \"0s\"", "stop_grace = \"9s\""),
        )
        .expect("rewrite");
        assert!(engine.plan(&file).expect("plan").is_noop());
    }

    #[test]
    fn invalid_update_values_are_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("stack.ctst");
        let backend = Arc::new(MemBackend::default());
        let engine = mem_engine(&backend, dir.path().join("data"));
        for (needle, replacement, expected) in [
            (
                "update = \"start-first\"",
                "update = \"blue-green\"",
                "invalid update strategy",
            ),
            (
                "stop_grace = \"0s\"",
                "stop_grace = \"soon\"",
                "invalid stop_grace",
            ),
        ] {
            std::fs::write(&file, ROLLING.replace(needle, replacement)).expect("write");
            let error = engine.plan(&file).expect_err("invalid");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn interrupted_update_after_handoff_is_completed_not_undone() {
        let backend = Arc::new(MemBackend::default());
        let (dir, file, engine) = deployed_rolling(&backend);
        let fingerprint = engine.plan(&file).expect("plan").changes[0]
            .fingerprint
            .clone()
            .expect("fingerprint");
        // Simulate a run killed while draining: web~next already serves.
        let staged = backend
            .create(&ContainerConfig {
                name: "web~next".into(),
                defer_publish: true,
                ..rolling_config()
            })
            .expect("stage");
        let _ = backend.start(&staged).expect("start");
        backend
            .handoff(&ContainerId::new("web-0"), &staged)
            .expect("handoff");
        let ledger_path = Ledger::path_for(&dir.path().join("data/state/state.json"));
        let mut ledger = Ledger::load(&ledger_path).expect("ledger");
        let _ = ledger.components.insert(
            "web~next".into(),
            AppliedComponent {
                fingerprint,
                container_id: staged.clone(),
            },
        );
        ledger.save(&ledger_path).expect("save");
        let _ = take_log(&backend);

        let outcome = engine.apply(&file, ApplyOptions::default()).expect("apply");

        assert_eq!(
            take_log(&backend),
            ["stop web-0", "remove web-0", "rename web~next-1 web"]
        );
        assert_eq!(outcome.deployed[0].id, staged);
        assert!(engine.plan(&file).expect("plan").is_noop());
    }

    #[test]
    fn concurrent_apply_is_refused() {
        let backend = Arc::new(MemBackend::default());
        let (dir, file, engine) = deployed_rolling(&backend);
        let held = ApplyLock::acquire(&dir.path().join("data/state/state.json")).expect("lock");
        let error = engine
            .apply(&file, ApplyOptions::default())
            .expect_err("locked");
        assert!(error.to_string().contains("another `ctst run`"), "{error}");
        drop(held);
        let _ = engine
            .apply(&file, ApplyOptions::default())
            .expect("apply after unlock");
    }

    #[test]
    fn duplicate_host_ports_across_components_are_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("stack.ctst");
        std::fs::write(
            &file,
            "COMPONENT a {\n image = \"file:///a\"\n ports = [8080]\n}\n\
             COMPONENT b {\n image = \"file:///b\"\n ports = [8080]\n}\n",
        )
        .expect("write");
        let backend = Arc::new(MemBackend::default());
        let error = mem_engine(&backend, dir.path().join("data"))
            .plan(&file)
            .expect_err("duplicate");
        assert!(error.to_string().contains("host port 8080"), "{error}");
    }

    #[test]
    fn connect_across_networks_is_flagged() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("stack.ctst");
        let backend = Arc::new(MemBackend::default());
        let engine = mem_engine(&backend, dir.path().join("data"));
        std::fs::write(&file, STACK).expect("write");
        let warnings = engine.plan(&file).expect("plan").warnings;
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("'api' CONNECTs to 'db'"));

        let shared = STACK
            .replace(
                "    port = 5432\n",
                "    port = 5432\n    network = \"app\"\n",
            )
            .replace(
                "    port = 8080\n",
                "    port = 8080\n    network = \"app\"\n",
            );
        std::fs::write(&file, shared).expect("write");
        assert_eq!(
            engine.plan(&file).expect("plan").warnings,
            Vec::<String>::new()
        );
    }

    #[test]
    fn repeating_a_port_in_expose_is_not_a_conflict() {
        use containust_common::types::PortMapping;

        let component = containust_compose::parser::ast::ComponentDecl {
            name: "web".into(),
            ports: vec![8080],
            ..Default::default()
        };
        let expose = |host, container| containust_compose::parser::ast::ExposeDecl {
            host_port: host,
            container_port: container,
        };
        let same = published_port_mappings(&component, &[expose(8080, 8080)]).expect("ok");
        assert_eq!(same, vec![PortMapping::identity(8080)]);
        let remap = published_port_mappings(&component, &[expose(80, 8080)]).expect("ok");
        assert_eq!(
            remap,
            vec![PortMapping {
                host: 80,
                container: 8080
            }]
        );
        let clash = published_port_mappings(
            &containust_compose::parser::ast::ComponentDecl {
                ports: vec![8080, 9090],
                ..component
            },
            &[expose(8080, 9090)],
        );
        assert!(
            clash.is_err(),
            "host 8080 cannot map to two container ports"
        );
    }

    #[test]
    fn durations_are_bounded() {
        assert_eq!(parse_duration_secs("24h"), Some(86_400));
        assert_eq!(parse_duration_secs("25h"), None);
        assert_eq!(parse_duration_secs("99999999999999999999h"), None);
        assert_eq!(parse_duration_secs("18446744073709551615h"), None);
    }

    fn rolling_config() -> ContainerConfig {
        ContainerConfig {
            name: String::new(),
            image: "file:///web/v2".into(),
            command: Vec::new(),
            env: Vec::new(),
            memory_bytes: None,
            cpu_shares: None,
            readonly_rootfs: true,
            volumes: Vec::new(),
            port: None,
            ports: Vec::new(),
            port_mappings: Vec::new(),
            network: "none".into(),
            restart: containust_common::types::RestartPolicy::Never,
            healthcheck: None,
            namespaces: containust_core::namespace::NamespaceConfig::default(),
            defer_publish: false,
        }
    }

    #[test]
    fn plan_on_fresh_project_does_not_create_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("stack.ctst");
        std::fs::write(&file, STACK).expect("write");
        let backend = Arc::new(MemBackend::default());
        let data_dir = dir.path().join("data");
        let engine = mem_engine(&backend, data_dir.clone());
        let plan = engine.plan(&file).expect("plan");
        assert_eq!(plan.count(PlanAction::Create), 2);
        assert!(!data_dir.exists());
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
    fn deploy_expose_remap_publishes_mapping() {
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

        let _ = engine.deploy(&file).expect("remap deploy");
        let config = state
            .config
            .lock()
            .expect("config lock")
            .clone()
            .expect("config captured");
        assert_eq!(
            config.port_mappings,
            vec![containust_common::types::PortMapping {
                host: 80,
                container: 8080,
            }]
        );
        assert!(config.namespaces.network);
        assert!(config.namespaces.user);
        assert_eq!(config.network, "none");
    }

    #[test]
    fn deploy_named_network_keeps_shared_netns() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("named-net.ctst");
        std::fs::write(
            &file,
            r#"COMPONENT web {
    image = "file:///unused"
    network = "backend"
}"#,
        )
        .expect("write composition");
        let state = Arc::new(FakeState::default());
        let engine = fake_engine(Arc::clone(&state), dir.path().join("data"), false);

        let _ = engine.deploy(&file).expect("named network deploy");
        let config = state
            .config
            .lock()
            .expect("config lock")
            .clone()
            .expect("config captured");
        assert_eq!(config.network, "backend");
        assert!(config.namespaces.network);
        // Shared netns is init-userns-owned; userns must stay off for setns.
        assert!(!config.namespaces.user);
        assert!(config.namespaces.pid);
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
