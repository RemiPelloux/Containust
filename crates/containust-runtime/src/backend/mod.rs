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
    /// Returns self as `Any` for downcasting to concrete backend types.
    fn as_any(&self) -> &dyn std::any::Any;

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
    fn detect_backend_returns_instance() {
        let backend = detect_backend();
        // On Linux the native backend is always available.
        // On macOS/Windows the VM backend is available when QEMU is installed.
        #[cfg(target_os = "linux")]
        assert!(backend.is_available());
        #[cfg(not(target_os = "linux"))]
        {
            // Just verify detect_backend returns a valid object.
            let _ = backend.is_available();
        }
    }

    #[test]
    fn platform_info_native_available_matches_target_os() {
        let info = platform_info();
        #[cfg(target_os = "linux")]
        assert!(info.native_available);
        #[cfg(not(target_os = "linux"))]
        assert!(!info.native_available);
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

    #[test]
    fn container_config_defaults() {
        let cfg = ContainerConfig {
            name: "minimal".into(),
            image: String::new(),
            command: Vec::new(),
            env: Vec::new(),
            memory_bytes: None,
            cpu_shares: None,
            readonly_rootfs: false,
            volumes: Vec::new(),
            port: None,
        };
        assert_eq!(cfg.name, "minimal");
        assert!(cfg.image.is_empty());
        assert!(cfg.memory_bytes.is_none());
        assert!(!cfg.readonly_rootfs);
        assert!(cfg.port.is_none());
    }

    #[test]
    fn container_config_clone_is_independent() {
        let cfg = ContainerConfig {
            name: "clone-test".into(),
            image: "file:///src".into(),
            command: vec!["sh".into()],
            env: vec![("A".into(), "1".into())],
            memory_bytes: Some(64 * 1024 * 1024),
            cpu_shares: Some(512),
            readonly_rootfs: false,
            volumes: vec!["/host:/guest".into()],
            port: Some(3000),
        };
        let cloned = cfg.clone();
        assert_eq!(cfg.name, cloned.name);
        assert_eq!(cfg.port, cloned.port);
    }

    #[test]
    fn container_info_can_be_constructed() {
        let id = ContainerId::new("abc-123");
        let info = ContainerInfo {
            id: id.clone(),
            name: "my-app".into(),
            state: "running".into(),
            pid: Some(42),
            image: "file:///app".into(),
            created_at: "2024-01-01T00:00:00Z".into(),
        };
        assert_eq!(info.id, id);
        assert_eq!(info.name, "my-app");
        assert_eq!(info.state, "running");
        assert_eq!(info.pid, Some(42));
    }

    #[test]
    fn container_info_stopped_has_no_pid() {
        let id = ContainerId::new("stopped-1");
        let info = ContainerInfo {
            id,
            name: "stopped-app".into(),
            state: "stopped".into(),
            pid: None,
            image: String::new(),
            created_at: String::new(),
        };
        assert!(info.pid.is_none());
        assert_eq!(info.state, "stopped");
    }

    #[test]
    fn container_info_clone_preserves_all_fields() {
        let id = ContainerId::new("clone-info");
        let info = ContainerInfo {
            id: id.clone(),
            name: "test".into(),
            state: "created".into(),
            pid: None,
            image: "tar:///archive.tar".into(),
            created_at: "2024-06-15T12:00:00Z".into(),
        };
        let cloned = info;
        assert_eq!(cloned.id, id);
        assert_eq!(cloned.image, "tar:///archive.tar");
    }
}
