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

pub mod initramfs;

const VM_PORT: u16 = 10809;
const VM_MEMORY_MB: u32 = 512;
const VM_CPUS: u32 = 2;
const VM_BOOT_TIMEOUT_SECS: u64 = 60;
const VM_POLL_INTERVAL_MS: u64 = 500;

const ALPINE_VERSION: &str = "3.21";

/// Backend that runs containers inside a lightweight Linux VM.
///
/// On macOS and Windows the kernel lacks native namespace/cgroup
/// support, so Containust boots a small Alpine Linux VM via QEMU
/// and delegates all container lifecycle operations to it.
pub struct VMBackend {
    vm_dir: PathBuf,
    vm_process: Mutex<Option<Child>>,
    forwarded_ports: Mutex<Vec<u16>>,
}

impl VMBackend {
    /// Creates a new VM backend.
    ///
    /// VM assets are stored in the global cache at `~/.containust/cache/vm/`.
    #[must_use]
    pub fn new() -> Self {
        let vm_dir = containust_common::constants::global_cache_dir().join("vm");
        Self {
            vm_dir,
            vm_process: Mutex::new(None),
            forwarded_ports: Mutex::new(Vec::new()),
        }
    }

    /// Ensures the VM assets (kernel + custom initramfs) are present on disk.
    /// Downloads Alpine Linux kernel and base initramfs on first run,
    /// then builds a custom initramfs with the Containust agent.
    ///
    /// # Errors
    ///
    /// Returns an error if downloads fail or the initramfs cannot be built.
    fn ensure_vm_assets(&self) -> Result<(PathBuf, PathBuf)> {
        std::fs::create_dir_all(&self.vm_dir).map_err(|e| ContainustError::Io {
            path: self.vm_dir.clone(),
            source: e,
        })?;

        let kernel_path = self.vm_dir.join("vmlinuz");
        let custom_initramfs_path = self.vm_dir.join("initramfs-containust.img");

        if !kernel_path.exists() || kernel_is_empty(&kernel_path) {
            download_kernel(&kernel_path)?;
        }

        // Always rebuild to pick up agent script changes
        let _ = std::fs::remove_file(&custom_initramfs_path);
        let base_initramfs_path = self.vm_dir.join("initramfs-base.img");
        if !base_initramfs_path.exists() {
            download_initramfs(&base_initramfs_path)?;
        }
        initramfs::build_initramfs(&base_initramfs_path, &custom_initramfs_path)?;

        Ok((kernel_path, custom_initramfs_path))
    }

    /// Boots the QEMU VM if it is not already running.
    ///
    /// # Errors
    ///
    /// Returns an error if QEMU is not installed, assets fail to download,
    /// or the VM fails to become reachable within the timeout.
    fn ensure_vm_running(&self, ports: &[u16]) -> Result<()> {
        let mut guard = lock_vm_process(&self.vm_process)?;

        if guard.is_some() {
            return Ok(());
        }

        let qemu = find_qemu()?;
        let (kernel, initramfs) = self.ensure_vm_assets()?;

        eprintln!("  Booting lightweight Linux VM...");
        let child = spawn_qemu(&qemu, &kernel, &initramfs, ports)?;

        *guard = Some(child);
        drop(guard);

        let mut port_guard = self.forwarded_ports.lock().map_err(|_| ContainustError::Config {
            message: "port list lock poisoned".into(),
        })?;
        port_guard.extend_from_slice(ports);
        drop(port_guard);

        wait_for_vm_ready()
    }

    /// Sends a JSON-RPC request to the in-VM agent and returns the response.
    ///
    /// # Errors
    ///
    /// Returns an error if the VM is unreachable, the request cannot be
    /// serialized, or the agent returns an error response.
    #[allow(clippy::unused_self)]
    fn send_command(&self, method: &str, params: &serde_json::Value) -> Result<serde_json::Value> {
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
        let ports_to_forward: Vec<u16> = std::iter::once(config.port)
            .flatten()
            .collect();

        self.ensure_vm_running(&ports_to_forward)?;

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
// Asset download helpers
// ---------------------------------------------------------------------------

/// Returns the Alpine Linux CDN architecture string.
const fn alpine_arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x86_64"
    }
}

/// Downloads the Alpine Linux netboot kernel.
fn download_kernel(dest: &Path) -> Result<()> {
    let arch = alpine_arch();
    let url = format!(
        "https://dl-cdn.alpinelinux.org/alpine/v{ALPINE_VERSION}/releases/{arch}/netboot/vmlinuz-virt"
    );
    eprintln!("  Downloading Alpine Linux kernel (first run only)...");
    download_file(&url, dest)
}

/// Downloads the Alpine Linux netboot initramfs.
fn download_initramfs(dest: &Path) -> Result<()> {
    let arch = alpine_arch();
    let url = format!(
        "https://dl-cdn.alpinelinux.org/alpine/v{ALPINE_VERSION}/releases/{arch}/netboot/initramfs-virt"
    );
    eprintln!("  Downloading Alpine Linux initramfs (first run only)...");
    download_file(&url, dest)
}

/// Downloads a file from `url` to `dest` with progress indication.
fn download_file(url: &str, dest: &Path) -> Result<()> {
    let response = reqwest::blocking::get(url).map_err(|e| ContainustError::Config {
        message: format!("failed to download {url}: {e}"),
    })?;

    if !response.status().is_success() {
        return Err(ContainustError::Config {
            message: format!("HTTP {} downloading {url}", response.status()),
        });
    }

    let total = response.content_length().unwrap_or(0);
    let bytes = response.bytes().map_err(|e| ContainustError::Config {
        message: format!("failed to read response body from {url}: {e}"),
    })?;

    if total > 0 {
        #[allow(clippy::cast_precision_loss)]
        let mb = bytes.len() as f64 / 1_048_576.0;
        eprintln!("  Downloaded {mb:.1} MB");
    }

    std::fs::write(dest, &bytes).map_err(|e| ContainustError::Io {
        path: dest.to_path_buf(),
        source: e,
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// QEMU helpers
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

/// Locks the VM process mutex, mapping a poisoned lock to a domain error.
fn lock_vm_process(
    mutex: &Mutex<Option<Child>>,
) -> Result<std::sync::MutexGuard<'_, Option<Child>>> {
    mutex.lock().map_err(|_| ContainustError::Config {
        message: "VM process lock poisoned".into(),
    })
}

/// Checks if a kernel file exists but is empty (a placeholder).
fn kernel_is_empty(kernel: &Path) -> bool {
    std::fs::metadata(kernel)
        .map(|m| m.len() == 0)
        .unwrap_or(true)
}

/// Spawns the QEMU process with all required arguments including dynamic port forwarding.
fn spawn_qemu(qemu: &Path, kernel: &Path, initramfs: &Path, ports: &[u16]) -> Result<Child> {
    tracing::info!(qemu = %qemu.display(), "booting VM");

    let mut hostfwd = format!("user,id=net0,hostfwd=tcp::{VM_PORT}-:{VM_PORT}");
    for &port in ports {
        if port != VM_PORT {
            use std::fmt::Write as _;
            let _ = write!(hostfwd, ",hostfwd=tcp::{port}-:{port}");
        }
    }

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
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    cmd.spawn().map_err(|e| ContainustError::Io {
        path: qemu.to_path_buf(),
        source: e,
    })
}

/// Sends a ping to the agent and checks for a pong response.
fn check_agent_ping(stream: &mut TcpStream) -> bool {
    let request = serde_json::json!({"method": "ping", "params": {}});
    let mut payload = serde_json::to_string(&request).unwrap_or_default();
    payload.push('\n');
    if stream.write_all(payload.as_bytes()).is_err() {
        return false;
    }
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).is_ok() && line.contains("pong")
}

/// Polls TCP until the VM agent is reachable or the timeout elapses.
fn wait_for_vm_ready() -> Result<()> {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(VM_BOOT_TIMEOUT_SECS);

    while start.elapsed() < timeout {
        if let Ok(mut stream) = TcpStream::connect(format!("127.0.0.1:{VM_PORT}")) {
            if check_agent_ping(&mut stream) {
                eprintln!("  VM is ready.");
                tracing::info!("VM is ready");
                return Ok(());
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(VM_POLL_INTERVAL_MS));
    }

    Err(ContainustError::Config {
        message: format!("VM failed to become reachable within {VM_BOOT_TIMEOUT_SECS}s"),
    })
}

/// Maximum RPC attempts before giving up.
const RPC_MAX_RETRIES: u32 = 8;
/// Delay between RPC retries.
const RPC_RETRY_DELAY_MS: u64 = 800;

/// Sends a single JSON-RPC request to the in-VM agent over TCP.
/// Retries on connection failure or empty responses.
fn send_rpc(method: &str, params: &serde_json::Value) -> Result<serde_json::Value> {
    let request = serde_json::json!({ "method": method, "params": params });
    let mut payload = serde_json::to_string(&request)?;
    payload.push('\n');

    let mut last_err = None;
    for attempt in 0..RPC_MAX_RETRIES {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(RPC_RETRY_DELAY_MS));
        }
        match try_send_rpc(&payload) {
            Ok(val) => {
                if let Some(error) = val.get("error") {
                    return Err(ContainustError::Config {
                        message: format!("VM agent error: {error}"),
                    });
                }
                return Ok(val);
            }
            Err(e) => {
                tracing::debug!(attempt, error = %e, "RPC attempt failed, retrying");
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| ContainustError::Config {
        message: "RPC failed after all retries".into(),
    }))
}

/// Single attempt to connect, send, and receive an RPC response.
fn try_send_rpc(payload: &str) -> Result<serde_json::Value> {
    let mut stream =
        TcpStream::connect(format!("127.0.0.1:{VM_PORT}")).map_err(|e| ContainustError::Io {
            path: PathBuf::from("VM agent"),
            source: e,
        })?;

    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));

    stream
        .write_all(payload.as_bytes())
        .map_err(|e| ContainustError::Io {
            path: PathBuf::from("VM agent"),
            source: e,
        })?;

    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    let _bytes = reader.read_line(&mut line).map_err(|e| ContainustError::Io {
        path: PathBuf::from("VM agent"),
        source: e,
    })?;

    if line.trim().is_empty() {
        return Err(ContainustError::Config {
            message: "empty response from VM agent".into(),
        });
    }

    serde_json::from_str(&line).map_err(Into::into)
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
