//! Cross-process VM lifecycle: pidfile, readiness adopt, graceful stop.

use std::path::Path;
use std::time::Duration;

use containust_common::error::{ContainustError, Result};
use containust_common::types::PortMapping;
use fs2::FileExt;

use super::pidfile::{VmPidRecord, clear_pid_record, write_pid_record};
use super::ports::{ensure_mappings_covered, normalize_forward_mappings, probe_available};
use super::process::{process_is_alive, terminate_pid, wait_until_dead};
use super::qemu::{QemuSpawn, find_qemu, spawn_qemu};
use super::rpc::{VM_AGENT_PORT, is_agent_ready, wait_for_vm_ready};

pub use super::pidfile::read_pid_record;

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
    ports: &[PortMapping],
) -> Result<VmStartOutcome> {
    let _lock = VmLock::acquire(vm_dir)?;
    let _ = recover_stale(vm_dir)?;
    let ports = normalize_forward_mappings(ports)?;

    if is_agent_ready() {
        return adopt_running_agent(vm_dir, &ports);
    }

    probe_available(&ports)?;
    let qemu = find_qemu()?;
    eprintln!("  Booting lightweight Linux VM...");
    let child = spawn_qemu(QemuSpawn {
        qemu: &qemu,
        kernel,
        initramfs,
        ports: &ports,
        vm_dir,
    })?;
    let pid = child.id();
    write_pid_record(
        vm_dir,
        &VmPidRecord {
            pid,
            agent_port: VM_AGENT_PORT,
            forwarded_ports: ports.iter().map(|m| m.host).collect(),
            forwarded_mappings: ports,
        },
    )?;
    // Detach: do not wait/kill on Child drop — the pidfile owns lifecycle.
    std::mem::forget(child);

    match wait_for_vm_ready() {
        Ok(()) => Ok(VmStartOutcome::Started),
        Err(error) => {
            let detail = super::qemu::read_stderr_tail(vm_dir);
            let _ = stop_running(vm_dir, true);
            Err(ContainustError::Config {
                message: format!("{error}; qemu stderr tail:\n{detail}"),
            })
        }
    }
}

fn adopt_running_agent(vm_dir: &Path, ports: &[PortMapping]) -> Result<VmStartOutcome> {
    if let Some(record) = read_pid_record(vm_dir)? {
        ensure_mappings_covered(&record.effective_mappings(), ports)?;
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
    Ok(VmStartOutcome::AlreadyRunning)
}

/// Stops the shared VM if a pidfile (or live agent) is present.
///
/// # Errors
///
/// Returns an error when stop cannot terminate a tracked process.
pub fn stop_running(vm_dir: &Path, force: bool) -> Result<()> {
    let _lock = VmLock::acquire(vm_dir)?;
    let Some(record) = read_pid_record(vm_dir)? else {
        if is_agent_ready() {
            return Err(ContainustError::Config {
                message: "VM agent is reachable but qemu.pid.json is missing; \
                     refuse to kill an untracked process. Restart the host or \
                     identify the QEMU PID manually"
                    .into(),
            });
        }
        return Ok(());
    };

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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::super::pidfile::write_pid_record;
    use super::*;

    #[test]
    fn recover_stale_clears_dead_pid() {
        let dir = tempfile::tempdir().unwrap();
        write_pid_record(
            dir.path(),
            &VmPidRecord {
                pid: 4_294_967_294,
                agent_port: 10809,
                forwarded_ports: vec![],
                forwarded_mappings: vec![],
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
