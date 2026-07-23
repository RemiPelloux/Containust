//! VM-based container backend for macOS and Windows.
//!
//! Boots a lightweight Alpine Linux VM via QEMU and forwards container
//! operations to the Linux native backend running inside it via a
//! JSON-RPC protocol over TCP.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use containust_common::error::{ContainustError, Result};
use containust_common::types::{ContainerId, PortMapping};

use super::{
    ContainerBackend, ContainerConfig, ContainerInfo, ReconciliationReport, project_identifier,
};
use crate::exec::ExecOutput;

pub mod assets;
mod assets_fetch;
pub mod initramfs;
mod lifecycle;
mod pidfile;
mod ports;
mod process;
mod protocol;
mod qemu;
mod response;
mod rpc;

/// Backend that runs containers inside a lightweight Linux VM via QEMU.
///
/// QEMU is tracked in `~/.containust/cache/vm/qemu.pid.json` so CLI
/// invocations can adopt/stop a shared VM; dropping `Engine` does not kill it.
pub struct VMBackend {
    vm_dir: PathBuf,
    data_dir: PathBuf,
    state_file: PathBuf,
    project_id: String,
    offline: bool,
    forwarded_ports: Mutex<Vec<u16>>,
}

impl VMBackend {
    /// Creates a new VM backend using default project paths.
    #[must_use]
    pub fn new() -> Self {
        let data_dir = containust_common::constants::project_dir(Path::new("containust.ctst"));
        let state_file = data_dir.join("state").join("state.json");
        Self::with_paths(data_dir, state_file)
    }

    /// Creates a VM backend scoped to explicit project storage paths.
    #[must_use]
    pub fn with_paths(data_dir: PathBuf, state_file: PathBuf) -> Self {
        Self::with_options(data_dir, state_file, false)
    }

    /// Creates a VM backend with storage paths and offline network policy.
    #[must_use]
    pub fn with_options(data_dir: PathBuf, state_file: PathBuf, offline: bool) -> Self {
        let vm_dir = containust_common::constants::global_cache_dir().join("vm");
        let project_id = project_identifier(&data_dir);
        Self {
            vm_dir,
            data_dir,
            state_file,
            project_id,
            offline,
            forwarded_ports: Mutex::new(Vec::new()),
        }
    }

    /// Returns the project data directory.
    #[must_use]
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Returns the project state file.
    #[must_use]
    pub fn state_file(&self) -> &Path {
        &self.state_file
    }

    /// Ensures pinned kernel + custom initramfs exist under the VM cache.
    ///
    /// # Errors
    ///
    /// Returns an error if download or initramfs build fails.
    fn ensure_vm_assets(&self) -> Result<(PathBuf, PathBuf)> {
        std::fs::create_dir_all(&self.vm_dir).map_err(|e| ContainustError::Io {
            path: self.vm_dir.clone(),
            source: e,
        })?;

        let kernel_path = self.vm_dir.join("vmlinuz");
        let custom_initramfs_path = self.vm_dir.join("initramfs-containust.img");
        let base_initramfs_path = self.vm_dir.join("initramfs-base.img");
        let entry = assets::asset_for_arch(assets::host_arch())?;
        assets::ensure_cached(
            entry,
            &kernel_path,
            &base_initramfs_path,
            assets::AssetCachePolicy {
                offline: self.offline,
            },
        )?;

        // Always rebuild to pick up agent script changes.
        let _ = std::fs::remove_file(&custom_initramfs_path);
        initramfs::build_initramfs(&base_initramfs_path, &custom_initramfs_path)?;

        Ok((kernel_path, custom_initramfs_path))
    }

    /// Boots the VM if needed (idempotent across CLI processes).
    ///
    /// # Errors
    ///
    /// Returns an error if QEMU, assets, or readiness polling fails.
    pub fn ensure_vm_running(&self, ports: &[PortMapping]) -> Result<()> {
        let (kernel, initramfs) = self.ensure_vm_assets()?;
        let outcome = lifecycle::ensure_running(&self.vm_dir, &kernel, &initramfs, ports)?;
        self.sync_forwarded_ports_from_pidfile()?;
        if matches!(outcome, lifecycle::VmStartOutcome::Started) {
            tracing::info!(?ports, "VM started with hostfwd ports");
        }
        Ok(())
    }

    /// Stops the shared VM (idempotent). `force` skips the SIGTERM grace period.
    ///
    /// # Errors
    ///
    /// Returns an error on lock/pidfile failure or an untracked live agent.
    pub fn stop_vm(&self, force: bool) -> Result<()> {
        lifecycle::stop_running(&self.vm_dir, force)?;
        self.forwarded_ports
            .lock()
            .map_err(|_| ContainustError::Config {
                message: "port list lock poisoned".into(),
            })?
            .clear();
        Ok(())
    }

    fn sync_forwarded_ports_from_pidfile(&self) -> Result<()> {
        let ports = lifecycle::read_pid_record(&self.vm_dir)?
            .map(|record| record.forwarded_ports)
            .unwrap_or_default();
        let mut guard = self
            .forwarded_ports
            .lock()
            .map_err(|_| ContainustError::Config {
                message: "port list lock poisoned".into(),
            })?;
        *guard = ports;
        drop(guard);
        Ok(())
    }

    fn send_command(&self, method: &str, params: &serde_json::Value) -> Result<serde_json::Value> {
        let mut scoped = params.clone();
        let object = scoped
            .as_object_mut()
            .ok_or_else(|| ContainustError::Config {
                message: "VM RPC parameters must be an object".into(),
            })?;
        let _ = object.insert("project".into(), self.project_id.clone().into());
        rpc::send_rpc(method, &scoped)
    }
}

impl Default for VMBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ContainerBackend for VMBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn create(&self, config: &ContainerConfig) -> Result<ContainerId> {
        let ports_to_forward = vm_forward_mappings(config);
        self.ensure_vm_running(&ports_to_forward)?;

        tracing::info!(name = %config.name, "creating container via VM backend");
        let response = self.send_command(
            "create",
            &serde_json::json!({
                "name": config.name,
                "image": config.image,
                "command": config.command,
                "env": config.env,
                "memory_bytes": config.memory_bytes,
                "cpu_shares": config.cpu_shares,
                "readonly_rootfs": config.readonly_rootfs,
                "volumes": config.volumes,
                "port": config.port,
                "ports": config.ports,
            }),
        )?;

        let id_str = response
            .get("result")
            .and_then(|r| r.get("id"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ContainustError::Config {
                message: "VM agent returned no container id".into(),
            })?;
        Ok(ContainerId::new(id_str))
    }

    fn start(&self, id: &ContainerId) -> Result<u32> {
        let response = self.send_command("start", &serde_json::json!({ "id": id.as_str() }))?;
        let pid = response
            .get("result")
            .and_then(|r| r.get("pid"))
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| ContainustError::Config {
                message: "VM agent returned no pid".into(),
            })?;
        response::truncate_u64_to_u32(pid)
    }

    fn stop(&self, id: &ContainerId) -> Result<()> {
        let response = self.send_command("stop", &serde_json::json!({ "id": id.as_str() }))?;
        response::expect_ok_result(&response)
    }

    fn exec(&self, id: &ContainerId, cmd: &[String]) -> Result<ExecOutput> {
        let response = self.send_command(
            "exec",
            &serde_json::json!({ "id": id.as_str(), "command": cmd }),
        )?;
        response::parse_exec_output(&response)
    }

    fn remove(&self, id: &ContainerId) -> Result<()> {
        let response = self.send_command("remove", &serde_json::json!({ "id": id.as_str() }))?;
        response::expect_ok_result(&response)
    }

    fn logs(&self, id: &ContainerId) -> Result<String> {
        let response = self.send_command("logs", &serde_json::json!({ "id": id.as_str() }))?;
        response::parse_logs(&response)
    }

    fn list(&self) -> Result<Vec<ContainerInfo>> {
        if !rpc::is_agent_ready() {
            return Ok(Vec::new());
        }
        let response = self.send_command("list", &serde_json::json!({}))?;
        let containers = response
            .get("result")
            .and_then(|r| r.get("containers"))
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(containers
            .iter()
            .filter_map(response::parse_container_info)
            .collect())
    }

    fn is_available(&self) -> bool {
        qemu::find_qemu().is_ok()
    }

    fn reconcile(&self) -> Result<ReconciliationReport> {
        let cleared = lifecycle::recover_stale(&self.vm_dir)?;
        Ok(ReconciliationReport {
            stale_processes: usize::from(cleared),
            ..ReconciliationReport::default()
        })
    }
}

/// Resolves QEMU hostfwd mappings from container config (remap-aware).
fn vm_forward_mappings(config: &ContainerConfig) -> Vec<PortMapping> {
    if !config.port_mappings.is_empty() {
        return config.port_mappings.clone();
    }
    let mut mappings: Vec<PortMapping> = config
        .ports
        .iter()
        .copied()
        .map(PortMapping::identity)
        .collect();
    if let Some(port) = config.port
        && !mappings.iter().any(|m| m.host == port)
    {
        mappings.push(PortMapping::identity(port));
    }
    mappings
}

#[cfg(test)]
mod tests {
    #![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

    use super::*;

    #[test]
    fn host_arch_is_supported() {
        let arch = assets::host_arch();
        assert!(matches!(arch, "aarch64" | "x86_64"));
        assert!(assets::asset_for_arch(arch).is_ok());
    }

    #[test]
    fn vm_backend_new_creates_instance() {
        let _ = VMBackend::default().is_available();
    }

    #[test]
    fn vm_backends_use_distinct_project_namespaces() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        let first = VMBackend::with_paths(a.clone(), a.join("state/state.json"));
        let second = VMBackend::with_paths(b.clone(), b.join("state/state.json"));
        assert_ne!(first.project_id, second.project_id);
        assert_eq!(first.data_dir(), a);
        assert_eq!(second.state_file(), b.join("state/state.json"));
        drop(VMBackend::new());
    }
}
