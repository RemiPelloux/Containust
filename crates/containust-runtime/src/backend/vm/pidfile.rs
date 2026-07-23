//! QEMU pidfile read/write under the VM cache directory.

use std::path::{Path, PathBuf};

use containust_common::error::{ContainustError, Result};
use containust_common::types::PortMapping;
use serde::{Deserialize, Serialize};

const PID_FILE_NAME: &str = "qemu.pid.json";

/// Persisted QEMU process metadata under the VM cache directory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VmPidRecord {
    /// Host PID of the QEMU process.
    pub pid: u32,
    /// Agent TCP port forwarded on the host.
    pub agent_port: u16,
    /// Host ports forwarded at QEMU boot (`hostfwd`) — identity view.
    #[serde(default)]
    pub forwarded_ports: Vec<u16>,
    /// Host→guest port mappings used for QEMU `hostfwd` (schema extension).
    #[serde(default)]
    pub forwarded_mappings: Vec<PortMapping>,
}

impl VmPidRecord {
    /// Effective forward mappings (legacy identity ports if mappings absent).
    #[must_use]
    pub fn effective_mappings(&self) -> Vec<PortMapping> {
        if self.forwarded_mappings.is_empty() {
            return self
                .forwarded_ports
                .iter()
                .copied()
                .map(PortMapping::identity)
                .collect();
        }
        self.forwarded_mappings.clone()
    }
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

pub(super) fn write_pid_record(vm_dir: &Path, record: &VmPidRecord) -> Result<()> {
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

pub(super) fn clear_pid_record(vm_dir: &Path) -> Result<()> {
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
            forwarded_mappings: vec![PortMapping::identity(8080), PortMapping::identity(8443)],
        };
        write_pid_record(dir.path(), &record).unwrap();
        let loaded = read_pid_record(dir.path()).unwrap().expect("present");
        assert_eq!(loaded, record);
        clear_pid_record(dir.path()).unwrap();
        assert!(read_pid_record(dir.path()).unwrap().is_none());
    }

    #[test]
    fn effective_mappings_falls_back_to_identity_ports() {
        let record = VmPidRecord {
            pid: 1,
            agent_port: 10809,
            forwarded_ports: vec![80],
            forwarded_mappings: vec![],
        };
        assert_eq!(record.effective_mappings(), vec![PortMapping::identity(80)]);
    }
}
