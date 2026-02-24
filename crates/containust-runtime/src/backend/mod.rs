//! Container backend abstraction for platform-agnostic operation.

pub mod linux;
pub mod vm;

use containust_common::error::Result;
use containust_common::types::ContainerId;

use crate::exec::ExecOutput;

/// Configuration for creating a container.
#[derive(Debug, Clone)]
pub struct ContainerConfig {
    /// Human-readable container name.
    pub name: String,
    /// Image source URI.
    pub image: String,
    /// Command to execute inside the container.
    pub command: Vec<String>,
    /// Environment variables.
    pub env: Vec<(String, String)>,
    /// Memory limit in bytes.
    pub memory_bytes: Option<u64>,
    /// CPU shares (relative weight).
    pub cpu_shares: Option<u64>,
    /// Whether the root filesystem is read-only.
    pub readonly_rootfs: bool,
    /// Volume mount specifications.
    pub volumes: Vec<String>,
    /// Primary exposed port.
    pub port: Option<u16>,
}

/// Information about a tracked container.
#[derive(Debug, Clone)]
pub struct ContainerInfo {
    /// Unique identifier.
    pub id: ContainerId,
    /// Human-readable name.
    pub name: String,
    /// Current state as a string.
    pub state: String,
    /// PID of the init process (if running).
    pub pid: Option<u32>,
    /// Image source URI.
    pub image: String,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
}

/// Platform-agnostic container backend.
///
/// Implementors handle the platform-specific details of container
/// creation, execution, and teardown.
pub trait ContainerBackend: Send + Sync {
    /// Creates a container from the given configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the container cannot be created.
    fn create(&self, config: &ContainerConfig) -> Result<ContainerId>;

    /// Starts a previously created container, returning its PID.
    ///
    /// # Errors
    ///
    /// Returns an error if the container cannot be started.
    fn start(&self, id: &ContainerId) -> Result<u32>;

    /// Stops a running container.
    ///
    /// # Errors
    ///
    /// Returns an error if the container cannot be stopped.
    fn stop(&self, id: &ContainerId) -> Result<()>;

    /// Executes a command inside a running container.
    ///
    /// # Errors
    ///
    /// Returns an error if the command fails to execute.
    fn exec(&self, id: &ContainerId, cmd: &[String]) -> Result<ExecOutput>;

    /// Removes a stopped container from the state.
    ///
    /// # Errors
    ///
    /// Returns an error if the container cannot be removed.
    fn remove(&self, id: &ContainerId) -> Result<()>;

    /// Returns the logs for a container.
    ///
    /// # Errors
    ///
    /// Returns an error if logs cannot be retrieved.
    fn logs(&self, id: &ContainerId) -> Result<String>;

    /// Lists all tracked containers.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend cannot retrieve state.
    fn list(&self) -> Result<Vec<ContainerInfo>>;

    /// Returns whether this backend is operational on the current platform.
    fn is_available(&self) -> bool;
}

/// Auto-detect and create the appropriate backend for the current platform.
#[must_use]
pub fn detect_backend() -> Box<dyn ContainerBackend> {
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxNativeBackend::new())
    }
    #[cfg(not(target_os = "linux"))]
    {
        Box::new(vm::VMBackend::new())
    }
}

/// Information about the current platform and backend availability.
#[derive(Debug, Clone)]
pub struct PlatformInfo {
    /// Host operating system name.
    pub os: String,
    /// Host CPU architecture.
    pub arch: String,
    /// Whether the Linux native backend is available.
    pub native_available: bool,
    /// Whether QEMU is installed for the VM backend.
    pub qemu_available: bool,
}

/// Returns information about the current platform and backend capabilities.
#[must_use]
pub fn platform_info() -> PlatformInfo {
    let qemu_binary = if cfg!(target_arch = "aarch64") {
        "qemu-system-aarch64"
    } else {
        "qemu-system-x86_64"
    };
    PlatformInfo {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        native_available: cfg!(target_os = "linux"),
        qemu_available: which::which(qemu_binary).is_ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_info_os_not_empty() {
        let info = platform_info();
        assert!(!info.os.is_empty());
    }

    #[test]
    fn platform_info_arch_not_empty() {
        let info = platform_info();
        assert!(!info.arch.is_empty());
    }

    #[test]
    fn detect_backend_returns_usable_backend() {
        let backend = detect_backend();
        #[cfg(target_os = "linux")]
        assert!(backend.is_available());
        #[cfg(not(target_os = "linux"))]
        assert!(!backend.is_available());
    }

    #[test]
    fn container_config_can_be_constructed() {
        let cfg = ContainerConfig {
            name: "test".into(),
            image: "file:///test".into(),
            command: vec!["echo".into()],
            env: vec![("KEY".into(), "val".into())],
            memory_bytes: Some(128 * 1024 * 1024),
            cpu_shares: None,
            readonly_rootfs: true,
            volumes: vec![],
            port: Some(8080),
        };
        assert_eq!(cfg.name, "test");
        assert!(cfg.readonly_rootfs);
    }
}
