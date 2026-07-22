//! QEMU discovery and process spawn for the VM backend.

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
        "virt"
    } else {
        "q35"
    }
}

/// Returns platform-specific QEMU acceleration flags.
#[must_use]
pub fn accel_flags() -> Vec<String> {
    if cfg!(target_os = "macos") {
        vec!["-accel".into(), "hvf".into()]
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

/// Spawns QEMU with agent and optional container port forwards.
///
/// # Errors
///
/// Returns an I/O error when the process cannot be spawned.
pub fn spawn_qemu(qemu: &Path, kernel: &Path, initramfs: &Path, ports: &[u16]) -> Result<Child> {
    tracing::info!(qemu = %qemu.display(), "booting VM");
    let hostfwd = build_netdev_arg(ports);

    let mut cmd = Command::new(qemu);
    let _ = cmd
        .args(["-machine", machine_type()])
        .args(accel_flags())
        .args(["-cpu", "max"])
        .args(["-kernel", &kernel.display().to_string()])
        .args(["-initrd", &initramfs.display().to_string()])
        .args(["-m", &VM_MEMORY_MB.to_string()])
        .args(["-smp", &VM_CPUS.to_string()])
        .arg("-nographic")
        .arg("-no-reboot")
        .args([
            "-append",
            if cfg!(target_arch = "aarch64") {
                "console=ttyAMA0 quiet loglevel=0"
            } else {
                "console=ttyS0 quiet loglevel=0"
            },
        ])
        .args(["-netdev", &hostfwd, "-device", "virtio-net-pci,netdev=net0"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    cmd.spawn().map_err(|e| ContainustError::Io {
        path: qemu.to_path_buf(),
        source: e,
    })
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
        assert!(matches!(machine_type(), "virt" | "q35"));
    }

    #[test]
    fn accel_flags_returns_non_empty() {
        let flags = accel_flags();
        assert!(!flags.is_empty());
        assert_eq!(flags[0], "-accel");
    }
}
