//! Linux native container backend using direct syscalls.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use containust_common::error::{ContainustError, Result};
use containust_common::types::ContainerId;

use super::{
    ContainerBackend, ContainerConfig, ContainerInfo, ReconciliationReport, project_identifier,
};
use crate::exec::ExecOutput;
use crate::process::terminate_process;
use crate::state::StateStore;

/// Backend that uses Linux kernel features directly.
///
/// Manages container state on disk and delegates process operations
/// to Linux namespaces, cgroups v2, `OverlayFS`, and `pivot_root`.
pub struct LinuxNativeBackend {
    data_dir: PathBuf,
    state_store: StateStore,
    project_id: String,
}

impl LinuxNativeBackend {
    /// Creates a new Linux native backend.
    #[must_use]
    pub fn new() -> Self {
        let data_dir =
            containust_common::constants::project_dir(std::path::Path::new("containust.ctst"));
        let state_file = data_dir.join("state").join("state.json");
        Self::with_paths(data_dir, state_file)
    }

    /// Creates a backend using explicit data and state paths.
    #[must_use]
    pub fn with_paths(data_dir: PathBuf, state_file: PathBuf) -> Self {
        let project_id = project_identifier(&data_dir);
        Self {
            data_dir,
            state_store: StateStore::new(state_file),
            project_id,
        }
    }

    /// Builds the initial persistent record for a freshly created container.
    fn new_state_entry(
        &self,
        id: &ContainerId,
        config: &ContainerConfig,
        rootfs: &Path,
    ) -> crate::state::StateEntry {
        crate::state::StateEntry {
            id: id.clone(),
            name: config.name.clone(),
            state: containust_common::types::ContainerState::Created,
            pid: None,
            image: config.image.clone(),
            command: config.command.clone(),
            env: containust_common::redact::redact_env(&config.env),
            memory_bytes: config.memory_bytes,
            cpu_shares: config.cpu_shares,
            readonly_rootfs: config.readonly_rootfs,
            volumes: config.volumes.clone(),
            rootfs_path: Some(rootfs.to_string_lossy().to_string()),
            ports: config.ports.clone(),
            port_mappings: config.port_mappings.clone(),
            network: config.network.clone(),
            forwarder_pids: Vec::new(),
            publish_deferred: config.defer_publish,
            restart: config.restart,
            healthcheck: config.healthcheck.clone(),
            health: config
                .healthcheck
                .as_ref()
                .map(|_| containust_common::types::HealthRecord::default()),
            log_path: Some(
                self.data_dir
                    .join("logs")
                    .join(format!("{id}.log"))
                    .to_string_lossy()
                    .to_string(),
            ),
            restart_count: 0,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

impl Default for LinuxNativeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ContainerBackend for LinuxNativeBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn create(&self, config: &ContainerConfig) -> Result<ContainerId> {
        let id = ContainerId::generate();
        tracing::info!(id = %id, name = %config.name, "creating container (Linux native)");

        // Cheap early name check under the shared lock; re-checked below.
        ensure_name_available(&self.state_store.read()?, &config.name)?;
        let _ = crate::volume::validate_volumes(&config.volumes)?;
        config.namespaces.validate_for_spawn()?;
        validate_resource_limits(config.memory_bytes, config.cpu_shares)?;

        // Build the rootfs (potentially slow: copy/extract/materialize)
        // without holding the exclusive state lock. It is built in a
        // staging directory outside `rootfs/` so a concurrent reconcile
        // cannot treat it as orphaned, then renamed into place while the
        // entry is committed.
        let staging = staging_rootfs_dir(&self.data_dir, &id);
        let rootfs = self.data_dir.join("rootfs").join(id.as_str());
        let store_result =
            prepare_rootfs_at(&self.data_dir, &config.image, &staging).and_then(|()| {
                self.state_store.update(|state| {
                    ensure_name_available(state, &config.name)?;
                    rename_dir(&staging, &rootfs)?;
                    state
                        .containers
                        .push(self.new_state_entry(&id, config, &rootfs));
                    Ok(())
                })
            });
        if let Err(error) = store_result {
            let _ = std::fs::remove_dir_all(&staging);
            let _ = std::fs::remove_dir_all(&rootfs);
            return Err(error);
        }
        tracing::info!(rootfs = %rootfs.display(), "rootfs prepared");
        Ok(id)
    }

    fn start(&self, id: &ContainerId) -> Result<u32> {
        tracing::info!(id = %id, "starting container (Linux native)");
        let start_result = self.state_store.update(|state| {
            let idx = state
                .containers
                .iter()
                .position(|entry| entry.id == *id)
                .ok_or_else(|| ContainustError::NotFound {
                    kind: "container",
                    id: id.as_str().to_string(),
                })?;
            let process_config = self.prepare_process_config(state, idx, id)?;
            let pid = match crate::process::spawn_container_process(&process_config) {
                Ok(pid) => pid,
                Err(error) => {
                    state.containers[idx].state = containust_common::types::ContainerState::Failed;
                    state.containers[idx].pid = None;
                    return Ok(Err(error));
                }
            };

            let entry = &mut state.containers[idx];
            let limits = containust_common::types::ResourceLimits {
                memory_bytes: entry.memory_bytes,
                cpu_shares: entry.cpu_shares,
                io_weight: None,
            };
            if let Err(error) = apply_cgroup_limits(&self.project_id, id, pid, &limits) {
                // Fail closed: tear down the just-spawned process. If kill
                // fails, keep the PID tracked so the orphan is not lost.
                entry.state = containust_common::types::ContainerState::Failed;
                return Ok(Err(fail_closed_after_cgroup_error(entry, pid, error)));
            }
            #[cfg(target_os = "linux")]
            {
                entry.forwarder_pids = start_entry_forwarders(&self.data_dir, entry, pid)?;
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = pid;
                entry.forwarder_pids.clear();
            }
            entry.state = containust_common::types::ContainerState::Running;
            entry.pid = Some(pid);
            Ok(Ok(pid))
        })?;
        let pid = start_result?;

        tracing::info!(pid, "container started (Linux native)");
        Ok(pid)
    }

    fn stop(&self, id: &ContainerId) -> Result<()> {
        tracing::info!(id = %id, "stopping container (Linux native)");

        self.stop_internal(id, false)
    }

    fn force_stop(&self, id: &ContainerId) -> Result<()> {
        self.stop_internal(id, true)
    }

    fn exec(&self, id: &ContainerId, cmd: &[String]) -> Result<ExecOutput> {
        let pid = self.running_pid_of(id)?;
        crate::exec::exec_in_container(id, pid, cmd)
    }

    #[cfg(target_os = "linux")]
    fn exec_timeout(
        &self,
        id: &ContainerId,
        cmd: &[String],
        timeout: std::time::Duration,
    ) -> Result<ExecOutput> {
        let pid = self.running_pid_of(id)?;
        crate::exec::exec_in_container_timeout(id, pid, cmd, Some(timeout))
    }

    fn remove(&self, id: &ContainerId) -> Result<()> {
        self.state_store.update(|state| {
            let index = state
                .containers
                .iter()
                .position(|entry| entry.id == *id)
                .ok_or_else(|| ContainustError::NotFound {
                    kind: "container",
                    id: id.to_string(),
                })?;
            if state.containers[index].state == containust_common::types::ContainerState::Running {
                return Err(ContainustError::Config {
                    message: format!("container {id} must be stopped before removal"),
                });
            }
            cleanup_container_files(&self.data_dir, &state.containers[index])?;
            #[cfg(target_os = "linux")]
            release_proc_netns(&self.data_dir, id.as_str());
            cleanup_cgroup(&self.project_id, id)?;
            let _ = state.containers.remove(index);
            Ok(())
        })?;
        Ok(())
    }

    fn logs(&self, id: &ContainerId) -> Result<String> {
        crate::logs::read_logs(&self.data_dir, id.as_str())
    }

    fn list(&self) -> Result<Vec<ContainerInfo>> {
        let state = self.state_store.read()?;
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

    fn reconcile(&self) -> Result<ReconciliationReport> {
        let (stale_processes, tracked_rootfs, tracked_ids) =
            self.state_store.update_if_changed(|state| {
                let (stale_processes, tracked_rootfs, tracked_ids) = reconcile_state_entries(state);
                Ok((
                    (stale_processes, tracked_rootfs, tracked_ids),
                    stale_processes > 0,
                ))
            })?;
        let orphaned_rootfs = cleanup_orphaned_rootfs(&self.data_dir, &tracked_rootfs)?;
        cleanup_stale_staging(&self.data_dir);
        let orphaned_cgroups = cleanup_orphaned_cgroups(&self.project_id, &tracked_ids);
        let policies = crate::supervise::enforce_policies(&self.state_store, self)?;
        Ok(ReconciliationReport {
            stale_processes,
            orphaned_rootfs,
            orphaned_cgroups,
            restarted: policies.restarted,
            unhealthy: policies.unhealthy,
        })
    }

    fn is_available(&self) -> bool {
        cfg!(target_os = "linux")
    }

    fn supports_handoff(&self) -> bool {
        cfg!(target_os = "linux")
    }

    #[cfg(target_os = "linux")]
    fn handoff(&self, from: &ContainerId, to: &ContainerId) -> Result<()> {
        self.state_store.update(|state| {
            let from_idx = entry_index(state, from)?;
            let to_idx = entry_index(state, to)?;
            let old_pids = std::mem::take(&mut state.containers[from_idx].forwarder_pids);
            let same_ports =
                state.containers[from_idx].port_mappings == state.containers[to_idx].port_mappings;
            let published =
                self.publish_staged(&mut state.containers[to_idx], same_ports, &old_pids);
            if let Err(error) = published {
                // Keep serving from the old container: re-point (or restart)
                // its forwarders and leave the staged one unpublished.
                state.containers[to_idx].publish_deferred = true;
                state.containers[from_idx].forwarder_pids =
                    self.restore_forwarders(&state.containers[from_idx], &old_pids);
                return Ok(Err(error));
            }
            Ok(Ok(()))
        })?
    }

    fn is_staged(&self, id: &ContainerId) -> Result<bool> {
        let state = self.state_store.read()?;
        let index = entry_index(&state, id)?;
        Ok(state.containers[index].publish_deferred)
    }

    fn rename(&self, id: &ContainerId, name: &str) -> Result<()> {
        self.state_store.update(|state| {
            if state
                .containers
                .iter()
                .any(|entry| entry.name == name && entry.id != *id)
            {
                return Err(ContainustError::Config {
                    message: format!("container name already exists: {name}"),
                });
            }
            let index = entry_index(state, id)?;
            state.containers[index].name = name.to_string();
            Ok(())
        })
    }
}

#[cfg(target_os = "linux")]
impl LinuxNativeBackend {
    /// Publishes the staged `entry`'s ports, taking over `old_pids` (the
    /// previous owner's forwarders) when the port set is unchanged.
    fn publish_staged(
        &self,
        entry: &mut crate::state::StateEntry,
        same_ports: bool,
        old_pids: &[u32],
    ) -> Result<()> {
        let pid = running_pid(entry).ok_or_else(|| ContainustError::Config {
            message: format!("container {} must be running to receive traffic", entry.id),
        })?;
        let mode = crate::network::NetworkMode::parse(Some(entry.network.as_str()));
        if entry.port_mappings.is_empty() || mode.is_host() {
            crate::port_forward::stop_forwarders_and_wait(old_pids);
            entry.publish_deferred = false;
            return Ok(());
        }
        let netns = entry_netns(&self.data_dir, entry, &mode, pid)?;
        let mappings = entry.port_mappings.clone();
        entry.forwarder_pids = if same_ports
            && crate::port_forward::forwarders_alive(old_pids)
            && crate::port_forward::targets_exist(&self.data_dir, &mappings)
        {
            crate::port_forward::retarget(&self.data_dir, &netns, &mappings)?;
            old_pids.to_vec()
        } else {
            tracing::warn!(id = %entry.id, "cannot retarget forwarders; restarting them (brief gap)");
            crate::port_forward::stop_forwarders_and_wait(old_pids);
            crate::port_forward::start_forwarders(&self.data_dir, &netns, &mappings)?
        };
        entry.publish_deferred = false;
        Ok(())
    }

    /// Best-effort: makes `entry` publish its ports again after a failed
    /// handoff, returning its forwarder PIDs.
    fn restore_forwarders(&self, entry: &crate::state::StateEntry, old_pids: &[u32]) -> Vec<u32> {
        let mode = crate::network::NetworkMode::parse(Some(entry.network.as_str()));
        let Some(pid) = running_pid(entry).filter(|_| !entry.port_mappings.is_empty()) else {
            return old_pids.to_vec();
        };
        if mode.is_host() {
            return old_pids.to_vec();
        }
        let restored = entry_netns(&self.data_dir, entry, &mode, pid).and_then(|netns| {
            if crate::port_forward::forwarders_alive(old_pids) {
                crate::port_forward::retarget(&self.data_dir, &netns, &entry.port_mappings)
                    .map(|()| old_pids.to_vec())
            } else {
                crate::port_forward::start_forwarders(&self.data_dir, &netns, &entry.port_mappings)
            }
        });
        restored.unwrap_or_else(|error| {
            tracing::error!(id = %entry.id, %error, "could not restore port forwarding");
            Vec::new()
        })
    }
}

fn entry_index(state: &crate::state::StateFile, id: &ContainerId) -> Result<usize> {
    state
        .containers
        .iter()
        .position(|entry| entry.id == *id)
        .ok_or_else(|| ContainustError::NotFound {
            kind: "container",
            id: id.to_string(),
        })
}

fn cleanup_container_files(data_dir: &Path, entry: &crate::state::StateEntry) -> Result<()> {
    let rootfs = data_dir.join("rootfs").join(entry.id.as_str());
    if rootfs.exists() {
        std::fs::remove_dir_all(&rootfs).map_err(|source| ContainustError::Io {
            path: rootfs,
            source,
        })?;
    }
    let log = data_dir
        .join("logs")
        .join(format!("{}.log", entry.id.as_str()));
    if log.exists() {
        std::fs::remove_file(&log).map_err(|source| ContainustError::Io { path: log, source })?;
    }
    Ok(())
}

fn cleanup_orphaned_rootfs(data_dir: &Path, tracked: &HashSet<PathBuf>) -> Result<usize> {
    let root = data_dir.join("rootfs");
    if !root.exists() {
        return Ok(0);
    }
    let mut removed = 0;
    for item in std::fs::read_dir(&root).map_err(|source| ContainustError::Io {
        path: root.clone(),
        source,
    })? {
        let item = item.map_err(|source| ContainustError::Io {
            path: root.clone(),
            source,
        })?;
        let path = item.path();
        if item
            .file_type()
            .map_err(|source| ContainustError::Io {
                path: path.clone(),
                source,
            })?
            .is_dir()
            && !tracked.contains(&path)
        {
            std::fs::remove_dir_all(&path)
                .map_err(|source| ContainustError::Io { path, source })?;
            removed += 1;
        }
    }
    Ok(removed)
}

fn reconcile_state_entries(
    state: &mut crate::state::StateFile,
) -> (usize, HashSet<PathBuf>, HashSet<String>) {
    let mut stale_processes = 0;
    for entry in &mut state.containers {
        if entry.state == containust_common::types::ContainerState::Running
            && entry.pid.is_none_or(|pid| !process_is_alive(pid))
        {
            entry.state = containust_common::types::ContainerState::Failed;
            entry.pid = None;
            stale_processes += 1;
        }
    }
    let tracked_rootfs = state
        .containers
        .iter()
        .filter_map(|entry| entry.rootfs_path.as_deref())
        .map(PathBuf::from)
        .collect();
    let tracked_ids = state
        .containers
        .iter()
        .map(|entry| entry.id.as_str().to_string())
        .collect();
    (stale_processes, tracked_rootfs, tracked_ids)
}

#[cfg(target_os = "linux")]
fn process_is_alive(pid: u32) -> bool {
    use nix::errno::Errno;
    use nix::sys::signal::kill;
    use nix::unistd::Pid;

    let pid = Pid::from_raw(i32::try_from(pid).unwrap_or(i32::MAX));
    match kill(pid, None) {
        Ok(()) | Err(Errno::EPERM) => true,
        Err(_) => false,
    }
}

#[cfg(not(target_os = "linux"))]
const fn process_is_alive(_pid: u32) -> bool {
    true
}

impl LinuxNativeBackend {
    /// PID of a tracked container's process, if it has one.
    fn running_pid_of(&self, id: &ContainerId) -> Result<u32> {
        let state = self.state_store.read()?;
        let entry = state
            .containers
            .iter()
            .find(|e| e.id == *id)
            .ok_or_else(|| ContainustError::NotFound {
                kind: "container",
                id: id.to_string(),
            })?;
        entry.pid.ok_or_else(|| ContainustError::Config {
            message: format!("container {id} is not running"),
        })
    }

    fn prepare_process_config(
        &self,
        state: &mut crate::state::StateFile,
        index: usize,
        id: &ContainerId,
    ) -> Result<crate::process::ProcessConfig> {
        let entry = &state.containers[index];
        if entry.state == containust_common::types::ContainerState::Running {
            return Err(ContainustError::Config {
                message: format!("container {id} is already running"),
            });
        }
        let image = entry.image.clone();
        let command = entry.command.clone();
        let env = containust_common::redact::resolve_env(&entry.env)
            .map_err(|message| ContainustError::Config { message })?;
        let readonly_rootfs = entry.readonly_rootfs;
        let volumes = entry.volumes.clone();
        let network = crate::network::NetworkMode::parse(Some(entry.network.as_str()));
        let mut namespaces =
            containust_core::namespace::NamespaceConfig::default().with_user_and_pid();
        namespaces.network = !network.is_host();
        // Shared netns lives in the init userns — cannot setns after NEWUSER.
        if network.shared_name().is_some() {
            namespaces.user = false;
        }
        let rootfs = match &entry.rootfs_path {
            Some(path) => PathBuf::from(path),
            None => prepare_rootfs(&self.data_dir, &image, id)?,
        };
        #[cfg(target_os = "linux")]
        let join_netns = prepare_network_for_start(&self.data_dir, state, &network, &rootfs)?;
        #[cfg(not(target_os = "linux"))]
        let join_netns = {
            let _ = (&network,);
            None
        };
        if state.containers[index].rootfs_path.is_none() {
            state.containers[index].rootfs_path = Some(rootfs.to_string_lossy().into_owned());
        }
        Ok(crate::process::ProcessConfig {
            command: if command.is_empty() {
                derive_command_from_image(&image)
            } else {
                command
            },
            env,
            rootfs,
            readonly_rootfs,
            volumes,
            namespaces,
            join_netns,
            log_path: Some(crate::logs::log_path(&self.data_dir, id.as_str())),
        })
    }

    fn stop_internal(&self, id: &ContainerId, force: bool) -> Result<()> {
        tracing::info!(id = %id, force, "stopping container (Linux native)");
        // Signal and wait for the process without holding the exclusive
        // state lock, so other project operations are not blocked for the
        // grace period.
        let observed_pid = {
            let state = self.state_store.read()?;
            let entry = state
                .containers
                .iter()
                .find(|entry| entry.id == *id)
                .ok_or_else(|| ContainustError::NotFound {
                    kind: "container",
                    id: id.to_string(),
                })?;
            running_pid(entry)
        };
        if let Some(pid) = observed_pid {
            terminate_process(pid, force, crate::process::STOP_GRACE);
        }
        self.state_store.update(|state| {
            let entry = state
                .containers
                .iter_mut()
                .find(|entry| entry.id == *id)
                .ok_or_else(|| ContainustError::NotFound {
                    kind: "container",
                    id: id.to_string(),
                })?;
            // The container may have been (re)started while unlocked; never
            // mark a live process stopped without signaling it.
            if let Some(pid) = running_pid(entry).filter(|pid| Some(*pid) != observed_pid) {
                terminate_process(pid, force, crate::process::STOP_GRACE);
            }
            #[cfg(target_os = "linux")]
            {
                crate::port_forward::stop_forwarders(&entry.forwarder_pids);
                entry.forwarder_pids.clear();
            }
            entry.state = containust_common::types::ContainerState::Stopped;
            entry.pid = None;
            Ok(())
        })?;
        cleanup_cgroup(&self.project_id, id)?;

        Ok(())
    }
}

/// Rejects a container name that is already tracked in `state`.
fn ensure_name_available(state: &crate::state::StateFile, name: &str) -> Result<()> {
    if state.containers.iter().any(|entry| entry.name == name) {
        return Err(ContainustError::Config {
            message: format!("container name already exists: {name}"),
        });
    }
    Ok(())
}

/// PID of the entry's process when it is tracked as running.
fn running_pid(entry: &crate::state::StateEntry) -> Option<u32> {
    entry
        .pid
        .filter(|_| entry.state == containust_common::types::ContainerState::Running)
}

// ---------------------------------------------------------------------------
// Image preparation helpers
// ---------------------------------------------------------------------------

/// Prepares a container rootfs at `{data_dir}/rootfs/{container_id}` from
/// the given image source URI.
///
/// Supported sources:
/// - `file://<path>` — bind-mounts or copies the directory as rootfs
/// - `tar://<path>` — extracts the archive into the rootfs directory
/// - `image://<name>[@sha256:<hex>]` — materializes an imported image
///   from the project's content-addressed catalog (offline-safe)
///
/// # Errors
///
/// Returns an error if the image source is unsupported or extraction fails.
fn prepare_rootfs(
    data_dir: &std::path::Path,
    image_uri: &str,
    container_id: &ContainerId,
) -> Result<PathBuf> {
    let rootfs_dir = data_dir.join("rootfs").join(container_id.as_str());

    // If rootfs already exists from a previous create, reuse it
    if rootfs_dir.exists() {
        tracing::info!(path = %rootfs_dir.display(), "reusing existing rootfs");
        return Ok(rootfs_dir);
    }
    prepare_rootfs_at(data_dir, image_uri, &rootfs_dir)?;
    Ok(rootfs_dir)
}

/// Directory where `create` builds a rootfs before committing it.
fn staging_rootfs_dir(data_dir: &Path, container_id: &ContainerId) -> PathBuf {
    data_dir
        .join(STAGING_ROOTFS_DIR)
        .join(container_id.as_str())
}

/// Parent of in-progress rootfs builds (never scanned by orphan cleanup
/// of `rootfs/`).
const STAGING_ROOTFS_DIR: &str = "rootfs-staging";

/// Staging directories older than this are treated as crash leftovers.
const STAGING_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// Atomically moves a fully built rootfs into its final location.
fn rename_dir(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ContainustError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::rename(from, to).map_err(|source| ContainustError::Io {
        path: to.to_path_buf(),
        source,
    })
}

/// Removes stale staging rootfs directories left by interrupted creates.
fn cleanup_stale_staging(data_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(data_dir.join(STAGING_ROOTFS_DIR)) else {
        return;
    };
    for entry in entries.flatten() {
        let stale = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age > STAGING_MAX_AGE);
        if stale {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// Builds a rootfs for `image_uri` into `rootfs_dir`.
fn prepare_rootfs_at(data_dir: &Path, image_uri: &str, rootfs_dir: &Path) -> Result<()> {
    if let Some(path_str) = image_uri.strip_prefix("file://") {
        let src = PathBuf::from(path_str);
        if !src.exists() {
            return Err(ContainustError::NotFound {
                kind: "image directory",
                id: path_str.to_string(),
            });
        }
        copy_dir_recursive(&src, rootfs_dir, rootfs_dir)?;
        tracing::info!(rootfs = %rootfs_dir.display(), "rootfs copied from file:// source");
    } else if let Some(path_str) = image_uri.strip_prefix("tar://") {
        let archive = PathBuf::from(path_str);
        if !archive.exists() {
            return Err(ContainustError::NotFound {
                kind: "tar archive",
                id: path_str.to_string(),
            });
        }
        extract_tar(&archive, rootfs_dir)?;
        tracing::info!(rootfs = %rootfs_dir.display(), "rootfs extracted from tar:// source");
    } else if image_uri.starts_with("image://") {
        let reference = containust_image::reference::ImageReference::parse(image_uri)?;
        containust_image::import::materialize_image(data_dir, &reference, rootfs_dir)?;
        tracing::info!(rootfs = %rootfs_dir.display(), "rootfs materialized from image catalog");
    } else {
        return Err(ContainustError::Config {
            message: format!("unsupported image source for Linux native: {image_uri}"),
        });
    }

    Ok(())
}

/// Copies a directory tree recursively without following symlinks.
/// Symlink targets must resolve under `confine_root`.
fn copy_dir_recursive(
    src: &std::path::Path,
    dst: &std::path::Path,
    confine_root: &std::path::Path,
) -> Result<()> {
    std::fs::create_dir_all(dst).map_err(|e| ContainustError::Io {
        path: dst.to_path_buf(),
        source: e,
    })?;

    for entry in std::fs::read_dir(src).map_err(|e| ContainustError::Io {
        path: src.to_path_buf(),
        source: e,
    })? {
        let entry = entry.map_err(|e| ContainustError::Io {
            path: src.to_path_buf(),
            source: e,
        })?;
        let file_type = entry.file_type().map_err(|e| ContainustError::Io {
            path: entry.path(),
            source: e,
        })?;
        let dest_path = dst.join(entry.file_name());
        copy_one_entry(&entry.path(), &dest_path, file_type, confine_root)?;
    }

    Ok(())
}

fn copy_one_entry(
    src: &std::path::Path,
    dest: &std::path::Path,
    file_type: std::fs::FileType,
    confine_root: &std::path::Path,
) -> Result<()> {
    if file_type.is_symlink() {
        return copy_symlink(src, dest, confine_root);
    }
    if file_type.is_dir() {
        return copy_dir_recursive(src, dest, confine_root);
    }
    containust_image::path_confine::assert_dest_confined(confine_root, dest)?;
    let _ = std::fs::copy(src, dest).map_err(|e| ContainustError::Io {
        path: src.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

fn copy_symlink(
    src: &std::path::Path,
    dest: &std::path::Path,
    confine_root: &std::path::Path,
) -> Result<()> {
    let link = std::fs::read_link(src).map_err(|e| ContainustError::Io {
        path: src.to_path_buf(),
        source: e,
    })?;
    containust_image::path_confine::ensure_symlink_confined(confine_root, dest, &link)?;
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&link, dest).map_err(|e| ContainustError::Io {
            path: dest.to_path_buf(),
            source: e,
        })
    }
    #[cfg(not(unix))]
    {
        let _ = (link, dest);
        Err(ContainustError::Config {
            message: "symlink copy requires a Unix host".into(),
        })
    }
}

/// After a cgroup apply failure, kill the process or keep its PID tracked.
fn fail_closed_after_cgroup_error(
    entry: &mut crate::state::StateEntry,
    pid: u32,
    error: ContainustError,
) -> ContainustError {
    match nix_kill(pid) {
        Ok(()) => {
            entry.pid = None;
            error
        }
        Err(kill_err) => {
            entry.pid = Some(pid);
            ContainustError::Config {
                message: format!(
                    "cgroup limits failed ({error}); also failed to kill pid {pid}: {kill_err}"
                ),
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn prepare_network_for_start(
    data_dir: &Path,
    state: &crate::state::StateFile,
    network: &crate::network::NetworkMode,
    rootfs: &Path,
) -> Result<Option<PathBuf>> {
    let join_netns = match network.shared_name() {
        Some(name) => Some(crate::network::ensure_shared_netns(data_dir, name)?),
        None => None,
    };
    if let Some(name) = network.shared_name() {
        let peers: Vec<String> = state
            .containers
            .iter()
            .filter(|c| c.network == name)
            .map(|c| c.name.clone())
            .collect();
        crate::network::write_container_hosts(rootfs, &peers)?;
    }
    Ok(join_netns)
}

/// Starts userspace forwarders when the container is not on the host network.
#[cfg(target_os = "linux")]
fn start_entry_forwarders(
    data_dir: &Path,
    entry: &crate::state::StateEntry,
    pid: u32,
) -> Result<Vec<u32>> {
    // A staged start-first replacement receives its ports on handoff.
    if entry.port_mappings.is_empty() || entry.publish_deferred {
        return Ok(Vec::new());
    }
    let mode = crate::network::NetworkMode::parse(Some(entry.network.as_str()));
    if mode.is_host() {
        return Ok(Vec::new());
    }
    let netns = entry_netns(data_dir, entry, &mode, pid)?;
    crate::port_forward::start_forwarders(data_dir, &netns, &entry.port_mappings)
}

/// Network namespace file that forwarders for `entry` relay into.
#[cfg(target_os = "linux")]
fn entry_netns(
    data_dir: &Path,
    entry: &crate::state::StateEntry,
    mode: &crate::network::NetworkMode,
    pid: u32,
) -> Result<PathBuf> {
    mode.shared_name().map_or_else(
        || persist_proc_netns(data_dir, entry.id.as_str(), pid),
        |name| crate::network::ensure_shared_netns(data_dir, name),
    )
}

/// Unmounts and deletes a container's persisted private netns (best-effort).
#[cfg(target_os = "linux")]
fn release_proc_netns(data_dir: &Path, container_id: &str) {
    let dir = data_dir.join("networks").join(container_id);
    let path = dir.join("ns");
    if path.exists() {
        unmount_all(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Removes every bind mount stacked on `path` (one per earlier start).
#[cfg(target_os = "linux")]
fn unmount_all(path: &Path) {
    for _ in 0..64 {
        if nix::mount::umount2(path, nix::mount::MntFlags::MNT_DETACH).is_err() {
            break;
        }
    }
}

#[cfg(target_os = "linux")]
fn persist_proc_netns(data_dir: &Path, container_id: &str, pid: u32) -> Result<PathBuf> {
    let path = data_dir.join("networks").join(container_id).join("ns");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ContainustError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    // A restarted container re-persists its new netns: drop the old bind
    // first (writing to a mounted nsfs file fails, and mounts would stack).
    unmount_all(&path);
    let _ = std::fs::File::create(&path).map_err(|source| ContainustError::Io {
        path: path.clone(),
        source,
    })?;
    let src = format!("/proc/{pid}/ns/net");
    let status = std::process::Command::new("mount")
        .args(["--bind", &src, &path.to_string_lossy()])
        .status()
        .map_err(|source| ContainustError::Io {
            path: PathBuf::from("mount"),
            source,
        })?;
    if !status.success() {
        return Err(ContainustError::Config {
            message: format!("failed to bind-mount {src} for port forwarding"),
        });
    }
    Ok(path)
}

/// Extracts a tar archive into a target directory with path-escape rejection.
fn extract_tar(archive: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    containust_image::extract::safe_extract_archive(archive, dst)
}

/// Derives a default shell command from the image source.
///
/// Defaults to `["sh"]` for images without a CMD manifest.
fn derive_command_from_image(_image_uri: &str) -> Vec<String> {
    vec!["sh".to_string()]
}

// ---------------------------------------------------------------------------
// Cgroup management helpers
// ---------------------------------------------------------------------------

/// Applies cgroup resource limits when any limit was explicitly requested.
/// Fail closed: if the caller asked for limits and they cannot be applied,
/// the container must not remain running.
fn apply_cgroup_limits(
    project_id: &str,
    container_id: &ContainerId,
    pid: u32,
    limits: &containust_common::types::ResourceLimits,
) -> Result<()> {
    let requested =
        limits.memory_bytes.is_some() || limits.cpu_shares.is_some() || limits.io_weight.is_some();
    if !requested {
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        use containust_core::cgroup::CgroupManager;

        let cgroup_id = format!("{project_id}/{}", container_id.as_str());
        let mgr = CgroupManager::create(&cgroup_id)?;
        mgr.apply_limits(limits)?;
        mgr.add_process(pid)?;
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (project_id, container_id, pid);
        Err(ContainustError::Config {
            message: "cgroup resource limits require Linux".into(),
        })
    }
}

/// Validates explicit resource limit ranges before create/start.
fn validate_resource_limits(memory_bytes: Option<u64>, cpu_shares: Option<u64>) -> Result<()> {
    if let Some(memory) = memory_bytes
        && memory == 0
    {
        return Err(ContainustError::Config {
            message: "memory limit must be greater than zero".into(),
        });
    }
    if let Some(cpu) = cpu_shares
        && !(1..=10_000).contains(&cpu)
    {
        return Err(ContainustError::Config {
            message: format!("cpu shares must be in 1..=10000, got {cpu}"),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn nix_kill(pid: u32) -> Result<()> {
    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;
    kill(Pid::from_raw(pid.cast_signed()), Signal::SIGKILL).map_err(|e| ContainustError::Config {
        message: format!("failed to kill pid {pid} after cgroup failure: {e}"),
    })
}

#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps, clippy::missing_const_for_fn)]
fn nix_kill(_pid: u32) -> Result<()> {
    Ok(())
}

/// Cgroup cleanup during container stop or removal.
fn cleanup_cgroup(project_id: &str, container_id: &ContainerId) -> Result<()> {
    let path = PathBuf::from(containust_common::constants::CGROUP_V2_PATH)
        .join("containust")
        .join(project_id)
        .join(container_id.as_str());
    if path.exists() {
        std::fs::remove_dir(&path).map_err(|source| ContainustError::Io {
            path: path.clone(),
            source,
        })?;
        tracing::debug!(path = %path.display(), "cgroup cleaned up");
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn cleanup_orphaned_cgroups(project_id: &str, tracked_ids: &HashSet<String>) -> usize {
    let root = PathBuf::from(containust_common::constants::CGROUP_V2_PATH)
        .join("containust")
        .join(project_id);
    let Ok(entries) = std::fs::read_dir(&root) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let id = entry.file_name().to_string_lossy().into_owned();
        if !tracked_ids.contains(&id) && cleanup_cgroup(project_id, &ContainerId::new(&id)).is_ok()
        {
            removed += 1;
        }
    }
    removed
}

#[cfg(not(target_os = "linux"))]
const fn cleanup_orphaned_cgroups(_project_id: &str, _tracked_ids: &HashSet<String>) -> usize {
    0
}

#[cfg(test)]
mod tests {
    #![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

    use super::*;

    fn test_state_entry(
        id: &str,
        state: containust_common::types::ContainerState,
        pid: Option<u32>,
        data_dir: &Path,
    ) -> crate::state::StateEntry {
        crate::state::StateEntry {
            id: ContainerId::new(id),
            name: id.into(),
            state,
            pid,
            image: "file:///image".into(),
            command: vec!["sh".into()],
            env: Vec::new(),
            memory_bytes: None,
            cpu_shares: None,
            readonly_rootfs: true,
            volumes: Vec::new(),
            rootfs_path: Some(
                data_dir
                    .join("rootfs")
                    .join(id)
                    .to_string_lossy()
                    .into_owned(),
            ),
            log_path: Some(
                data_dir
                    .join("logs")
                    .join(format!("{id}.log"))
                    .to_string_lossy()
                    .into_owned(),
            ),
            ports: Vec::new(),
            port_mappings: Vec::new(),
            network: "bridge".into(),
            forwarder_pids: Vec::new(),
            publish_deferred: false,
            restart: containust_common::types::RestartPolicy::default(),
            healthcheck: None,
            health: None,
            restart_count: 0,
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn derive_command_returns_default_sh() {
        let cmd = derive_command_from_image("file:///some/image");
        assert_eq!(cmd, vec!["sh"]);
    }

    #[test]
    fn derive_command_empty_uri_returns_sh() {
        let cmd = derive_command_from_image("");
        assert_eq!(cmd, vec!["sh"]);
    }

    #[test]
    fn derive_command_tar_uri_returns_sh() {
        let cmd = derive_command_from_image("tar:///archive.tar.gz");
        assert_eq!(cmd, vec!["sh"]);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_native_backend_new_creates_instance() {
        let backend = LinuxNativeBackend::new();
        assert!(backend.is_available());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_native_backend_default_creates_instance() {
        let backend = LinuxNativeBackend::default();
        assert!(backend.is_available());
    }

    #[test]
    fn copy_dir_recursive_copies_files() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("containust_test_copy");
        let src = temp_dir.join("src");
        let dst = temp_dir.join("dst");

        // Setup
        std::fs::create_dir_all(&src).expect("create src dir");
        std::fs::write(src.join("file.txt"), "hello").expect("write file");
        std::fs::create_dir_all(src.join("sub")).expect("create subdir");
        std::fs::write(src.join("sub").join("nested.txt"), "nested").expect("write nested file");

        copy_dir_recursive(&src, &dst, &dst)?;

        assert!(dst.join("file.txt").exists());
        assert!(dst.join("sub").join("nested.txt").exists());
        assert_eq!(
            std::fs::read_to_string(dst.join("file.txt")).expect("read"),
            "hello"
        );
        assert_eq!(
            std::fs::read_to_string(dst.join("sub").join("nested.txt")).expect("read"),
            "nested"
        );

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn copy_dir_recursive_empty_directory() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("containust_test_empty_copy");
        let src = temp_dir.join("empty_src");
        let dst = temp_dir.join("empty_dst");

        std::fs::create_dir_all(&src).expect("create src");
        copy_dir_recursive(&src, &dst, &dst)?;

        assert!(dst.exists());
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn cleanup_cgroup_does_not_panic_on_missing_dir() {
        let id = ContainerId::new("nonexistent-cgroup");
        assert!(cleanup_cgroup("nonexistent-project", &id).is_ok());
    }

    #[test]
    fn terminate_process_does_not_panic_on_invalid_pid() {
        // Use a PID that almost certainly does not exist
        terminate_process(999_999_999, true, crate::process::STOP_GRACE);
    }

    #[test]
    fn stop_retains_rootfs_logs_and_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let data_dir = dir.path().join("project");
        let state_file = data_dir.join("state").join("state.json");
        let backend = LinuxNativeBackend::with_paths(data_dir.clone(), state_file);
        let entry = test_state_entry(
            "retained",
            containust_common::types::ContainerState::Created,
            None,
            &data_dir,
        );
        let rootfs = data_dir.join("rootfs").join("retained");
        let log = data_dir.join("logs").join("retained.log");
        std::fs::create_dir_all(&rootfs).expect("rootfs");
        std::fs::create_dir_all(log.parent().expect("log parent")).expect("logs");
        std::fs::write(&log, "logs").expect("log");
        backend
            .state_store
            .write(&crate::state::StateFile {
                containers: vec![entry],
                ..crate::state::StateFile::default()
            })
            .expect("state");

        backend.stop(&ContainerId::new("retained")).expect("stop");

        assert!(rootfs.exists());
        assert!(log.exists());
        let state = backend.state_store.read().expect("read state");
        assert_eq!(
            state.containers[0].state,
            containust_common::types::ContainerState::Stopped
        );
    }

    #[test]
    fn remove_deletes_project_owned_resources() {
        let dir = tempfile::tempdir().expect("tempdir");
        let data_dir = dir.path().join("project");
        let state_file = data_dir.join("state").join("state.json");
        let backend = LinuxNativeBackend::with_paths(data_dir.clone(), state_file);
        let entry = test_state_entry(
            "removed",
            containust_common::types::ContainerState::Stopped,
            None,
            &data_dir,
        );
        let rootfs = data_dir.join("rootfs").join("removed");
        let log = data_dir.join("logs").join("removed.log");
        std::fs::create_dir_all(&rootfs).expect("rootfs");
        std::fs::create_dir_all(log.parent().expect("log parent")).expect("logs");
        std::fs::write(&log, "logs").expect("log");
        backend
            .state_store
            .write(&crate::state::StateFile {
                containers: vec![entry],
                ..crate::state::StateFile::default()
            })
            .expect("state");

        backend
            .remove(&ContainerId::new("removed"))
            .expect("remove");

        assert!(!rootfs.exists());
        assert!(!log.exists());
        assert!(
            backend
                .state_store
                .read()
                .expect("read state")
                .containers
                .is_empty()
        );
    }

    #[test]
    fn remove_rejects_running_container() {
        let dir = tempfile::tempdir().expect("tempdir");
        let data_dir = dir.path().join("project");
        let backend = LinuxNativeBackend::with_paths(
            data_dir.clone(),
            data_dir.join("state").join("state.json"),
        );
        backend
            .state_store
            .write(&crate::state::StateFile {
                containers: vec![test_state_entry(
                    "running",
                    containust_common::types::ContainerState::Running,
                    Some(1),
                    &data_dir,
                )],
                ..crate::state::StateFile::default()
            })
            .expect("state");

        let error = backend
            .remove(&ContainerId::new("running"))
            .expect_err("running remove");
        assert!(error.to_string().contains("must be stopped"));
    }

    #[test]
    fn reconcile_removes_orphaned_rootfs() {
        let dir = tempfile::tempdir().expect("tempdir");
        let data_dir = dir.path().join("project");
        let orphan = data_dir.join("rootfs").join("orphan");
        std::fs::create_dir_all(&orphan).expect("orphan");
        let backend = LinuxNativeBackend::with_paths(
            data_dir.clone(),
            data_dir.join("state").join("state.json"),
        );

        let report = backend.reconcile().expect("reconcile");
        assert_eq!(report.orphaned_rootfs, 1);
        assert!(!orphan.exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn reconcile_marks_dead_running_process_failed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let data_dir = dir.path().join("project");
        let backend = LinuxNativeBackend::with_paths(
            data_dir.clone(),
            data_dir.join("state").join("state.json"),
        );
        backend
            .state_store
            .write(&crate::state::StateFile {
                containers: vec![test_state_entry(
                    "dead",
                    containust_common::types::ContainerState::Running,
                    Some(999_999_999),
                    &data_dir,
                )],
                ..crate::state::StateFile::default()
            })
            .expect("state");

        let report = backend.reconcile().expect("reconcile");
        assert_eq!(report.stale_processes, 1);
        let state = backend.state_store.read().expect("state");
        assert_eq!(
            state.containers[0].state,
            containust_common::types::ContainerState::Failed
        );
        assert!(state.containers[0].pid.is_none());
    }

    #[test]
    fn state_stores_are_project_scoped() {
        let dir = tempfile::tempdir().expect("tempdir");
        let first_dir = dir.path().join("first");
        let second_dir = dir.path().join("second");
        let first = LinuxNativeBackend::with_paths(
            first_dir.clone(),
            first_dir.join("state").join("state.json"),
        );
        let second = LinuxNativeBackend::with_paths(
            second_dir.clone(),
            second_dir.join("state").join("state.json"),
        );
        first
            .state_store
            .write(&crate::state::StateFile {
                containers: vec![test_state_entry(
                    "first-only",
                    containust_common::types::ContainerState::Stopped,
                    None,
                    &first_dir,
                )],
                ..crate::state::StateFile::default()
            })
            .expect("first state");

        assert_eq!(first.list().expect("first list").len(), 1);
        assert!(second.list().expect("second list").is_empty());
    }

    #[test]
    fn create_commits_rootfs_and_rejects_duplicate_name_without_leaks() {
        let dir = tempfile::tempdir().expect("tempdir");
        let image = dir.path().join("image");
        std::fs::create_dir_all(image.join("bin")).expect("image");
        std::fs::write(image.join("bin/app"), "binary").expect("image file");
        let data_dir = dir.path().join("project");
        let backend =
            LinuxNativeBackend::with_paths(data_dir.clone(), data_dir.join("state/state.json"));
        let config = ContainerConfig {
            name: "dup".into(),
            image: format!("file://{}", image.display()),
            command: vec!["/bin/app".into()],
            env: Vec::new(),
            memory_bytes: None,
            cpu_shares: None,
            readonly_rootfs: true,
            volumes: Vec::new(),
            port: None,
            ports: Vec::new(),
            port_mappings: Vec::new(),
            network: "bridge".into(),
            restart: containust_common::types::RestartPolicy::default(),
            healthcheck: None,
            namespaces: containust_core::namespace::NamespaceConfig::default(),
            defer_publish: false,
        };

        let id = backend.create(&config).expect("create");
        let rootfs = data_dir.join("rootfs").join(id.as_str());
        assert!(rootfs.join("bin/app").exists());
        let state = backend.state_store.read().expect("state");
        assert_eq!(
            state.containers[0].rootfs_path.as_deref(),
            Some(rootfs.to_string_lossy().as_ref())
        );

        let error = backend.create(&config).expect_err("duplicate name");
        assert!(error.to_string().contains("already exists"));
        let rootfs_dirs = std::fs::read_dir(data_dir.join("rootfs"))
            .expect("rootfs dir")
            .count();
        assert_eq!(rootfs_dirs, 1);
        let staging_left =
            std::fs::read_dir(data_dir.join(STAGING_ROOTFS_DIR)).map_or(0, Iterator::count);
        assert_eq!(staging_left, 0);
    }

    #[test]
    fn two_projects_create_and_cleanup_independently() {
        let dir = tempfile::tempdir().expect("tempdir");
        let image = dir.path().join("image");
        std::fs::create_dir_all(image.join("bin")).expect("image");
        std::fs::write(image.join("bin/app"), "binary").expect("image file");
        let first_dir = dir.path().join("first/.containust");
        let second_dir = dir.path().join("second/.containust");
        let first =
            LinuxNativeBackend::with_paths(first_dir.clone(), first_dir.join("state/state.json"));
        let second =
            LinuxNativeBackend::with_paths(second_dir.clone(), second_dir.join("state/state.json"));
        let config = ContainerConfig {
            name: "app".into(),
            image: format!("file://{}", image.display()),
            command: vec!["/bin/app".into()],
            env: Vec::new(),
            memory_bytes: None,
            cpu_shares: None,
            readonly_rootfs: true,
            volumes: Vec::new(),
            port: None,
            ports: Vec::new(),
            port_mappings: Vec::new(),
            network: "bridge".into(),
            restart: containust_common::types::RestartPolicy::default(),
            healthcheck: None,
            namespaces: containust_core::namespace::NamespaceConfig::default(),
            defer_publish: false,
        };

        let first_id = first.create(&config).expect("first create");
        let second_id = second.create(&config).expect("second create");
        crate::logs::append_log(&first_dir, first_id.as_str(), "first").expect("first log");
        crate::logs::append_log(&second_dir, second_id.as_str(), "second").expect("second log");

        assert_eq!(first.list().expect("first list").len(), 1);
        assert_eq!(second.list().expect("second list").len(), 1);
        first.remove(&first_id).expect("first remove");
        assert!(first.list().expect("first empty").is_empty());
        assert_eq!(second.list().expect("second retained").len(), 1);
        assert!(second_dir.join("rootfs").join(second_id.as_str()).exists());
        assert!(
            crate::logs::read_logs(&second_dir, second_id.as_str())
                .expect("second logs")
                .contains("second")
        );
    }
}
