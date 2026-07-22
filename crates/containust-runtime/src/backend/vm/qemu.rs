//! QEMU discovery and process spawn for the VM backend.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use containust_common::error::{ContainustError, Result};

use super::ports::build_netdev_arg;

const VM_MEMORY_MB: u32 = 512;
const VM_CPUS: u32 = 2;

/// Returns the QEMU binary name for the host architecture.
#[must_use]
pub const fn qemu_binary_name() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "qemu-system-aarch64"
    } else {
        "qemu-system-x86_64"
    }
}

/// Returns the QEMU machine type for the host architecture.
#[must_use]
pub const fn machine_type() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "virt,gic-version=3"
    } else {
        "q35"
    }
}

/// Returns the preferred CPU model for acceleration.
#[must_use]
pub const fn cpu_model() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "host"
    } else {
        "max"
    }
}

/// Returns the virtio NIC model for the host architecture.
#[must_use]
pub const fn net_device() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "virtio-net-device,netdev=net0"
    } else {
        "virtio-net-pci,netdev=net0"
    }
}

/// Returns platform-specific QEMU acceleration flags (preferred then fallback).
#[must_use]
pub fn accel_flags() -> Vec<String> {
    if cfg!(target_os = "macos") {
        vec!["-accel".into(), "hvf".into(), "-accel".into(), "tcg".into()]
    } else if cfg!(target_os = "windows") {
        vec!["-accel".into(), "whpx,kernel-irqchip=off".into()]
    } else {
        vec!["-accel".into(), "tcg".into()]
    }
}

/// Finds the QEMU binary for the current architecture.
///
/// # Errors
///
/// Returns [`ContainustError::NotFound`] when QEMU is not on `PATH`.
pub fn find_qemu() -> Result<PathBuf> {
    let binary = qemu_binary_name();
    which::which(binary).map_err(|_| {
        let install_hint = if cfg!(target_os = "macos") {
            "Install with: brew install qemu"
        } else if cfg!(target_os = "windows") {
            "Install with: choco install qemu"
        } else {
            "Install with: apt install qemu-system"
        };
        ContainustError::NotFound {
            kind: "QEMU binary",
            id: format!("{binary} — {install_hint}"),
        }
    })
}

/// Path where QEMU stderr is captured for diagnostics.
#[must_use]
pub fn qemu_stderr_path(vm_dir: &Path) -> PathBuf {
    vm_dir.join("qemu.stderr.log")
}

/// Inputs for a QEMU VM spawn.
#[derive(Debug, Clone, Copy)]
pub struct QemuSpawn<'a> {
    /// Absolute path to the QEMU binary.
    pub qemu: &'a Path,
    /// Linux kernel image.
    pub kernel: &'a Path,
    /// Initramfs with the Containust agent.
    pub initramfs: &'a Path,
    /// Extra hostfwd ports (agent port is always included).
    pub ports: &'a [u16],
    /// VM state directory (for stderr capture).
    pub vm_dir: &'a Path,
}

/// Spawns QEMU with agent and optional container port forwards.
///
/// # Errors
///
/// Returns an I/O error when the process or log file cannot be created.
pub fn spawn_qemu(opts: QemuSpawn<'_>) -> Result<Child> {
    tracing::info!(qemu = %opts.qemu.display(), "booting VM");
    let hostfwd = build_netdev_arg(opts.ports);
    let stderr_path = qemu_stderr_path(opts.vm_dir);
    let stderr_file = File::create(&stderr_path).map_err(|source| ContainustError::Io {
        path: stderr_path.clone(),
        source,
    })?;

    let mut cmd = Command::new(opts.qemu);
    let _ = cmd
        .args(["-machine", machine_type()])
        .args(accel_flags())
        .args(["-cpu", cpu_model()])
        .args(["-kernel", &opts.kernel.display().to_string()])
        .args(["-initrd", &opts.initramfs.display().to_string()])
        .args(["-m", &VM_MEMORY_MB.to_string()])
        .args(["-smp", &VM_CPUS.to_string()])
        .arg("-nographic")
        .arg("-no-reboot")
        .args([
            "-append",
            if cfg!(target_arch = "aarch64") {
                "console=ttyAMA0 earlyprintk=serial,ttyAMA0 loglevel=3"
            } else {
                "console=ttyS0 earlyprintk=serial,ttyS0 loglevel=3"
            },
        ])
        .args(["-netdev", &hostfwd, "-device", net_device()])
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr_file));

    cmd.spawn().map_err(|e| ContainustError::Io {
        path: opts.qemu.to_path_buf(),
        source: e,
    })
}

/// Returns a short tail of the QEMU stderr log for error messages.
#[must_use]
pub fn read_stderr_tail(vm_dir: &Path) -> String {
    let path = qemu_stderr_path(vm_dir);
    let Ok(content) = std::fs::read_to_string(&path) else {
        return String::new();
    };
    let lines: Vec<&str> = content.lines().rev().take(12).collect();
    if lines.is_empty() {
        return String::new();
    }
    let mut ordered: Vec<&str> = lines.into_iter().rev().collect();
    ordered.dedup();
    format!("; qemu stderr: {}", ordered.join(" | "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qemu_binary_name_is_valid() {
        assert!(matches!(
            qemu_binary_name(),
            "qemu-system-aarch64" | "qemu-system-x86_64"
        ));
    }

    #[test]
    fn machine_type_is_valid() {
        assert!(machine_type().starts_with("virt") || machine_type() == "q35");
    }

    #[test]
    fn accel_flags_returns_non_empty() {
        let flags = accel_flags();
        assert!(!flags.is_empty());
        assert_eq!(flags[0], "-accel");
    }

    #[test]
    fn net_device_mentions_netdev() {
        assert!(net_device().contains("netdev=net0"));
    }
}
