//! Cross-process VM lifecycle: pidfile, readiness adopt, graceful stop.

use std::path::{Path, PathBuf};
use std::time::Duration;

use containust_common::error::{ContainustError, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

use super::ports::{ensure_ports_covered, normalize_forward_ports, probe_available};
use super::process::{process_is_alive, terminate_pid, wait_until_dead};
use super::qemu::{find_qemu, spawn_qemu};
use super::rpc::{VM_AGENT_PORT, is_agent_ready, wait_for_vm_ready};

const PID_FILE_NAME: &str = "qemu.pid.json";
const LOCK_FILE_NAME: &str = ".vm.lock";
const FORCE_WAIT: Duration = Duration::from_secs(2);

/// Outcome of an idempotent VM start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmStartOutcome {
    /// A new QEMU process was spawned and became ready.
    Started,
    /// An existing agent (and optionally pidfile) was adopted.
    AlreadyRunning,
}

/// Persisted QEMU process metadata under the VM cache directory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VmPidRecord {
    /// Host PID of the QEMU process.
    pub pid: u32,
    /// Agent TCP port forwarded on the host.
    pub agent_port: u16,
    /// Container host ports forwarded at QEMU boot (`hostfwd`).
    #[serde(default)]
    pub forwarded_ports: Vec<u16>,
}

/// Exclusive lock for VM start/stop races.
struct VmLock {
    file: std::fs::File,
}

impl VmLock {
    fn acquire(vm_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(vm_dir).map_err(|source| ContainustError::Io {
            path: vm_dir.to_path_buf(),
            source,
        })?;
        let path = vm_dir.join(LOCK_FILE_NAME);
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|source| ContainustError::Io {
                path: path.clone(),
                source,
            })?;
        FileExt::lock_exclusive(&file).map_err(|source| ContainustError::Io { path, source })?;
        Ok(Self { file })
    }
}

impl Drop for VmLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

/// Ensures a ready VM exists, adopting a live agent or spawning QEMU.
///
/// # Errors
///
/// Returns an error when QEMU cannot be found, spawn fails, or readiness times out.
pub fn ensure_running(
    vm_dir: &Path,
    kernel: &Path,
    initramfs: &Path,
    ports: &[u16],
) -> Result<VmStartOutcome> {
    let _lock = VmLock::acquire(vm_dir)?;
    let _ = recover_stale(vm_dir)?;
    let ports = normalize_forward_ports(ports)?;

    if is_agent_ready() {
        if let Some(record) = read_pid_record(vm_dir)? {
            ensure_ports_covered(&record.forwarded_ports, &ports)?;
        } else {
            tracing::warn!("VM agent is ready but pidfile is missing; continuing");
            if !ports.is_empty() {
                return Err(ContainustError::Config {
                    message: "VM agent is reachable without qemu.pid.json; refuse to \
                         assume hostfwd ownership. Run `ctst vm stop` and restart"
                        .into(),
                });
            }
        }
        return Ok(VmStartOutcome::AlreadyRunning);
    }

    probe_available(&ports)?;
    let qemu = find_qemu()?;
    eprintln!("  Booting lightweight Linux VM...");
    let child = spawn_qemu(&qemu, kernel, initramfs, &ports)?;
    let pid = child.id();
    write_pid_record(
        vm_dir,
        &VmPidRecord {
            pid,
            agent_port: VM_AGENT_PORT,
            forwarded_ports: ports,
        },
    )?;
    // Detach: do not wait/kill on Child drop — the pidfile owns lifecycle.
    std::mem::forget(child);

    match wait_for_vm_ready() {
        Ok(()) => Ok(VmStartOutcome::Started),
        Err(error) => {
            terminate_pid(pid, true);
            clear_pid_record(vm_dir)?;
            Err(error)
        }
    }
}

/// Stops the shared VM if running. Idempotent when already stopped.
///
/// When `force` is false, sends SIGTERM and waits briefly before SIGKILL.
///
/// # Errors
///
/// Returns an error when the lifecycle lock or pidfile I/O fails.
pub fn stop_running(vm_dir: &Path, force: bool) -> Result<()> {
    let _lock = VmLock::acquire(vm_dir)?;
    let record = read_pid_record(vm_dir)?;
    let agent_up = is_agent_ready();

    let Some(record) = record else {
        if agent_up {
            return Err(ContainustError::Config {
                message: "VM agent is reachable but qemu.pid.json is missing; \
                     refuse to kill an untracked process. Remove the orphan \
                     manually or recreate the pidfile"
                    .into(),
            });
        }
        tracing::info!("VM already stopped");
        return Ok(());
    };

    if !process_is_alive(record.pid) && !agent_up {
        clear_pid_record(vm_dir)?;
        tracing::info!("VM already stopped (stale pidfile cleared)");
        return Ok(());
    }

    terminate_pid(record.pid, force);
    if process_is_alive(record.pid) {
        terminate_pid(record.pid, true);
        let _ = wait_until_dead(record.pid, FORCE_WAIT);
    }
    clear_pid_record(vm_dir)?;
    tracing::info!(pid = record.pid, force, "VM stopped");
    Ok(())
}

/// Removes a dead pidfile entry. Returns `true` when a stale record was cleared.
///
/// # Errors
///
/// Returns an I/O error when the pidfile cannot be read or removed.
pub fn recover_stale(vm_dir: &Path) -> Result<bool> {
    let Some(record) = read_pid_record(vm_dir)? else {
        return Ok(false);
    };
    if process_is_alive(record.pid) || is_agent_ready() {
        if process_is_alive(record.pid) && !is_agent_ready() {
            tracing::warn!(
                pid = record.pid,
                "QEMU alive but agent not ready; leaving pidfile for stop/retry"
            );
        }
        return Ok(false);
    }
    clear_pid_record(vm_dir)?;
    tracing::info!(pid = record.pid, "cleared stale VM pidfile");
    Ok(true)
}

/// Reads the VM pidfile if present.
///
/// # Errors
///
/// Returns an error when the file exists but cannot be parsed.
pub fn read_pid_record(vm_dir: &Path) -> Result<Option<VmPidRecord>> {
    let path = pid_path(vm_dir);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).map_err(|source| ContainustError::Io {
        path: path.clone(),
        source,
    })?;
    let record: VmPidRecord = serde_json::from_str(&raw).map_err(|source| {
        tracing::error!(path = %path.display(), %source, "invalid VM pidfile");
        ContainustError::Serialization { source }
    })?;
    Ok(Some(record))
}

fn write_pid_record(vm_dir: &Path, record: &VmPidRecord) -> Result<()> {
    std::fs::create_dir_all(vm_dir).map_err(|source| ContainustError::Io {
        path: vm_dir.to_path_buf(),
        source,
    })?;
    let path = pid_path(vm_dir);
    let body = serde_json::to_string_pretty(record)?;
    let staging = path.with_extension("json.tmp");
    std::fs::write(&staging, body).map_err(|source| ContainustError::Io {
        path: staging.clone(),
        source,
    })?;
    std::fs::rename(&staging, &path).map_err(|source| ContainustError::Io { path, source })?;
    Ok(())
}

fn clear_pid_record(vm_dir: &Path) -> Result<()> {
    let path = pid_path(vm_dir);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|source| ContainustError::Io { path, source })?;
    }
    Ok(())
}

fn pid_path(vm_dir: &Path) -> PathBuf {
    vm_dir.join(PID_FILE_NAME)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;

    #[test]
    fn pid_record_roundtrip_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let record = VmPidRecord {
            pid: 4242,
            agent_port: 10809,
            forwarded_ports: vec![8080, 8443],
        };
        write_pid_record(dir.path(), &record).unwrap();
        let loaded = read_pid_record(dir.path()).unwrap().expect("present");
        assert_eq!(loaded, record);
        clear_pid_record(dir.path()).unwrap();
        assert!(read_pid_record(dir.path()).unwrap().is_none());
    }

    #[test]
    fn recover_stale_clears_dead_pid() {
        let dir = tempfile::tempdir().unwrap();
        write_pid_record(
            dir.path(),
            &VmPidRecord {
                pid: 4_294_967_294,
                agent_port: 10809,
                forwarded_ports: vec![],
            },
        )
        .unwrap();
        if process_is_alive(4_294_967_294) {
            return;
        }
        assert!(recover_stale(dir.path()).unwrap());
        assert!(read_pid_record(dir.path()).unwrap().is_none());
    }

    #[test]
    fn stop_running_without_pidfile_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        stop_running(dir.path(), true).unwrap();
    }
}
