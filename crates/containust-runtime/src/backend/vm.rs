//! VM-based container backend for macOS and Windows.
//!
//! Boots a lightweight Alpine Linux VM via QEMU and forwards container
//! operations to the Linux native backend running inside it via a
//! JSON-RPC protocol over TCP.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use containust_common::error::{ContainustError, Result};
use containust_common::types::ContainerId;

use super::{ContainerBackend, ContainerConfig, ContainerInfo};
use crate::exec::ExecOutput;

const VM_PORT: u16 = 10809;
const VM_MEMORY_MB: u32 = 512;
const VM_CPUS: u32 = 2;
const VM_BOOT_TIMEOUT_SECS: u64 = 30;
const VM_POLL_INTERVAL_MS: u64 = 500;

/// Backend that runs containers inside a lightweight Linux VM.
///
/// On macOS and Windows the kernel lacks native namespace/cgroup
/// support, so Containust boots a small Alpine Linux VM via QEMU
/// and delegates all container lifecycle operations to it.
pub struct VMBackend {
    vm_dir: PathBuf,
    vm_process: Mutex<Option<Child>>,
}

impl VMBackend {
    /// Creates a new VM backend.
    ///
    /// The VM assets directory defaults to `~/.containust/vm/`.
    #[must_use]
    pub fn new() -> Self {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| "/tmp".into());
        Self {
            vm_dir: PathBuf::from(home).join(".containust").join("vm"),
            vm_process: Mutex::new(None),
        }
    }

    /// Ensures the VM assets (kernel + initramfs) are present on disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created or
    /// placeholder files cannot be written.
    fn ensure_vm_assets(&self) -> Result<(PathBuf, PathBuf)> {
        std::fs::create_dir_all(&self.vm_dir).map_err(|e| ContainustError::Io {
            path: self.vm_dir.clone(),
            source: e,
        })?;

        let kernel_path = self.vm_dir.join("vmlinuz");
        let initramfs_path = self.vm_dir.join("initramfs.img");

        if !kernel_path.exists() || !initramfs_path.exists() {
            self.create_placeholder_assets(&kernel_path, &initramfs_path)?;
        }

        Ok((kernel_path, initramfs_path))
    }

    /// Writes empty placeholder files plus a README with download instructions.
    fn create_placeholder_assets(&self, kernel: &Path, initramfs: &Path) -> Result<()> {
        let readme_path = self.vm_dir.join("README.md");
        let readme_content = concat!(
            "# Containust VM Assets\n\n",
            "To run containers on macOS or Windows, Containust needs a ",
            "lightweight Linux VM.\n\n",
            "## Quick Setup\n\n",
            "Download the Alpine Linux virtual kernel and initramfs:\n\n",
            "```bash\n",
            "# For x86_64\n",
            "curl -L -o vmlinuz ",
            "https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/",
            "x86_64/netboot/vmlinuz-virt\n",
            "curl -L -o initramfs.img ",
            "https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/",
            "x86_64/netboot/initramfs-virt\n\n",
            "# For aarch64\n",
            "curl -L -o vmlinuz ",
            "https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/",
            "aarch64/netboot/vmlinuz-virt\n",
            "curl -L -o initramfs.img ",
            "https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/",
            "aarch64/netboot/initramfs-virt\n",
            "```\n\n",
            "Place both files in this directory: `~/.containust/vm/`\n",
        );

        write_file(&readme_path, readme_content)?;
        write_file(kernel, "")?;
        write_file(initramfs, "")?;

        tracing::info!(
            dir = %self.vm_dir.display(),
            "VM asset placeholders created — see README.md for setup"
        );
        Ok(())
    }

    /// Boots the QEMU VM if it is not already running.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - QEMU is not installed
    /// - VM assets are missing (empty placeholders)
    /// - The VM fails to become reachable within the timeout
    fn ensure_vm_running(&self) -> Result<()> {
        let mut guard = lock_vm_process(&self.vm_process)?;

        if guard.is_some() {
            return Ok(());
        }

        let qemu = find_qemu()?;
        let (kernel, initramfs) = self.ensure_vm_assets()?;

        validate_kernel_not_empty(&kernel)?;

        let child = spawn_qemu(&qemu, &kernel, &initramfs)?;

        *guard = Some(child);
        drop(guard);

        wait_for_vm_ready()
    }

    /// Sends a JSON-RPC request to the in-VM agent and returns the response.
    ///
    /// # Errors
    ///
    /// Returns an error if the VM is unreachable, the request cannot be
    /// serialized, or the agent returns an error response.
    fn send_command(&self, method: &str, params: &serde_json::Value) -> Result<serde_json::Value> {
        self.ensure_vm_running()?;
        send_rpc(method, params)
    }

    /// Stops the VM process if it is running.
    ///
    /// # Errors
    ///
    /// Returns an error if the process mutex is poisoned.
    pub fn stop_vm(&self) -> Result<()> {
        let mut guard = lock_vm_process(&self.vm_process)?;

        if let Some(mut child) = guard.take() {
            drop(guard);
            let _ = child.kill();
            let _ = child.wait();
            tracing::info!("VM stopped");
        }

        Ok(())
    }
}

impl Default for VMBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ContainerBackend for VMBackend {
    fn create(&self, config: &ContainerConfig) -> Result<ContainerId> {
        tracing::info!(
            name = %config.name,
            "creating container via VM backend"
        );
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
        truncate_u64_to_u32(pid)
    }

    fn stop(&self, id: &ContainerId) -> Result<()> {
        let _response = self.send_command("stop", &serde_json::json!({ "id": id.as_str() }))?;
        Ok(())
    }

    fn exec(&self, id: &ContainerId, cmd: &[String]) -> Result<ExecOutput> {
        let response = self.send_command(
            "exec",
            &serde_json::json!({ "id": id.as_str(), "command": cmd }),
        )?;
        Ok(parse_exec_output(&response))
    }

    fn remove(&self, id: &ContainerId) -> Result<()> {
        let _response = self.send_command("remove", &serde_json::json!({ "id": id.as_str() }))?;
        Ok(())
    }

    fn logs(&self, id: &ContainerId) -> Result<String> {
        let response = self.send_command("logs", &serde_json::json!({ "id": id.as_str() }))?;
        let logs = response
            .get("result")
            .and_then(|r| r.get("logs"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        Ok(logs.to_string())
    }

    fn list(&self) -> Result<Vec<ContainerInfo>> {
        let guard = lock_vm_process(&self.vm_process)?;
        if guard.is_none() {
            return Ok(Vec::new());
        }
        drop(guard);

        let response = self.send_command("list", &serde_json::json!({}))?;
        let containers = response
            .get("result")
            .and_then(|r| r.get("containers"))
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();

        Ok(containers.iter().filter_map(parse_container_info).collect())
    }

    fn is_available(&self) -> bool {
        find_qemu().is_ok()
    }
}

impl Drop for VMBackend {
    fn drop(&mut self) {
        let _ = self.stop_vm();
    }
}

// ---------------------------------------------------------------------------
// Free helper functions
// ---------------------------------------------------------------------------

/// Returns the QEMU binary name for the host architecture.
const fn qemu_binary_name() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "qemu-system-aarch64"
    } else {
        "qemu-system-x86_64"
    }
}

/// Returns the QEMU machine type for the host architecture.
const fn machine_type() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "virt"
    } else {
        "q35"
    }
}

/// Returns platform-specific QEMU acceleration flags.
fn accel_flags() -> Vec<String> {
    if cfg!(target_os = "macos") {
        vec!["-accel".into(), "hvf".into()]
    } else if cfg!(target_os = "windows") {
        vec!["-accel".into(), "whpx,kernel-irqchip=off".into()]
    } else {
        vec!["-accel".into(), "tcg".into()]
    }
}

/// Finds the QEMU binary for the current architecture.
fn find_qemu() -> Result<PathBuf> {
    let binary = qemu_binary_name();
    which::which(binary).map_err(|_| ContainustError::NotFound {
        kind: "QEMU binary",
        id: format!("{binary} (install QEMU to use containers on this platform)"),
    })
}

/// Locks the VM process mutex, mapping a poisoned lock to a domain error.
fn lock_vm_process(
    mutex: &Mutex<Option<Child>>,
) -> Result<std::sync::MutexGuard<'_, Option<Child>>> {
    mutex.lock().map_err(|_| ContainustError::Config {
        message: "VM process lock poisoned".into(),
    })
}

/// Validates that the kernel file is not an empty placeholder.
fn validate_kernel_not_empty(kernel: &Path) -> Result<()> {
    let meta = std::fs::metadata(kernel).map_err(|e| ContainustError::Io {
        path: kernel.to_path_buf(),
        source: e,
    })?;
    if meta.len() == 0 {
        return Err(ContainustError::Config {
            message: format!(
                "VM kernel not found. Download Alpine Linux kernel to {}. \
                 See ~/.containust/vm/README.md for instructions.",
                kernel.display()
            ),
        });
    }
    Ok(())
}

/// Writes `contents` to `path`, mapping I/O errors to the domain type.
fn write_file(path: &Path, contents: &str) -> Result<()> {
    std::fs::write(path, contents).map_err(|e| ContainustError::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

/// Spawns the QEMU process with all required arguments.
fn spawn_qemu(qemu: &Path, kernel: &Path, initramfs: &Path) -> Result<Child> {
    tracing::info!(qemu = %qemu.display(), "booting VM");

    Command::new(qemu)
        .args(["-machine", machine_type()])
        .args(accel_flags())
        .args(["-kernel", &kernel.display().to_string()])
        .args(["-initrd", &initramfs.display().to_string()])
        .args(["-m", &VM_MEMORY_MB.to_string()])
        .args(["-smp", &VM_CPUS.to_string()])
        .arg("-nographic")
        .arg("-no-reboot")
        .args([
            "-append",
            &format!("console=ttyS0 quiet containust_port={VM_PORT}"),
        ])
        .args([
            "-netdev",
            &format!("user,id=net0,hostfwd=tcp::{VM_PORT}-:{VM_PORT}"),
            "-device",
            "virtio-net-pci,netdev=net0",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ContainustError::Io {
            path: qemu.to_path_buf(),
            source: e,
        })
}

/// Polls TCP until the VM agent is reachable or the timeout elapses.
fn wait_for_vm_ready() -> Result<()> {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(VM_BOOT_TIMEOUT_SECS);

    while start.elapsed() < timeout {
        if TcpStream::connect(format!("127.0.0.1:{VM_PORT}")).is_ok() {
            tracing::info!("VM is ready");
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(VM_POLL_INTERVAL_MS));
    }

    Err(ContainustError::Config {
        message: format!("VM failed to become reachable within {VM_BOOT_TIMEOUT_SECS}s"),
    })
}

/// Sends a single JSON-RPC request to the in-VM agent over TCP.
fn send_rpc(method: &str, params: &serde_json::Value) -> Result<serde_json::Value> {
    let mut stream =
        TcpStream::connect(format!("127.0.0.1:{VM_PORT}")).map_err(|e| ContainustError::Io {
            path: PathBuf::from("VM agent"),
            source: e,
        })?;

    let request = serde_json::json!({
        "method": method,
        "params": params,
    });

    let mut payload = serde_json::to_string(&request)?;
    payload.push('\n');

    stream
        .write_all(payload.as_bytes())
        .map_err(|e| ContainustError::Io {
            path: PathBuf::from("VM agent"),
            source: e,
        })?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let _bytes = reader
        .read_line(&mut line)
        .map_err(|e| ContainustError::Io {
            path: PathBuf::from("VM agent"),
            source: e,
        })?;

    let response: serde_json::Value = serde_json::from_str(&line)?;

    if let Some(error) = response.get("error") {
        return Err(ContainustError::Config {
            message: format!("VM agent error: {error}"),
        });
    }

    Ok(response)
}

/// Safely converts a `u64` to `u32`, returning an error on overflow.
fn truncate_u64_to_u32(value: u64) -> Result<u32> {
    u32::try_from(value).map_err(|_| ContainustError::Config {
        message: format!("PID value {value} exceeds u32 range"),
    })
}

/// Extracts `ExecOutput` fields from a VM agent response.
fn parse_exec_output(response: &serde_json::Value) -> ExecOutput {
    let result = response.get("result").cloned().unwrap_or_default();
    let stdout = result
        .get("stdout")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let stderr = result
        .get("stderr")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let raw_code = result
        .get("exit_code")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(-1);
    let exit_code = i32::try_from(raw_code).unwrap_or(-1);

    ExecOutput {
        stdout,
        stderr,
        exit_code,
    }
}

/// Parses a JSON value from the VM agent into a `ContainerInfo`.
fn parse_container_info(value: &serde_json::Value) -> Option<ContainerInfo> {
    let pid_u64 = value.get("pid").and_then(serde_json::Value::as_u64);
    let pid = pid_u64.and_then(|v| u32::try_from(v).ok());
    Some(ContainerInfo {
        id: ContainerId::new(value.get("id")?.as_str()?),
        name: value.get("name")?.as_str()?.to_string(),
        state: value.get("state")?.as_str()?.to_string(),
        pid,
        image: value.get("image")?.as_str()?.to_string(),
        created_at: value.get("created_at")?.as_str()?.to_string(),
    })
}
