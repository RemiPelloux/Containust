//! Linux namespace management for container isolation.
//!
//! Provides safe wrappers around `clone(2)`, `unshare(2)`, and `setns(2)`
//! for each namespace type.

pub mod ipc;
pub mod mount;
pub mod network;
pub mod pid;
pub mod user;
pub mod uts;

use containust_common::error::{ContainustError, Result};

/// Configuration for which namespaces to create or join.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct NamespaceConfig {
    /// Isolate PID namespace.
    pub pid: bool,
    /// Isolate mount namespace.
    pub mount: bool,
    /// Isolate network namespace.
    pub network: bool,
    /// Isolate user namespace.
    pub user: bool,
    /// Isolate IPC namespace.
    pub ipc: bool,
    /// Isolate UTS (hostname) namespace.
    pub uts: bool,
}

impl Default for NamespaceConfig {
    fn default() -> Self {
        Self {
            pid: true,
            mount: true,
            network: true,
            user: true,
            ipc: true,
            uts: true,
        }
    }
}

/// Creates all configured namespaces for a new container.
///
/// Uses `unshare(2)` to detach the calling process from the specified
/// namespace types, giving it isolated views of the corresponding resources.
///
/// # Errors
///
/// Returns an error if any namespace creation syscall fails, or if
/// running on a non-Linux platform.
#[cfg(target_os = "linux")]
pub fn create_namespaces(config: &NamespaceConfig) -> Result<()> {
    use nix::sched::{CloneFlags, unshare};

    let mut flags = CloneFlags::empty();
    if config.user {
        flags |= CloneFlags::CLONE_NEWUSER;
    }
    if config.mount {
        flags |= CloneFlags::CLONE_NEWNS;
    }
    if config.pid {
        flags |= CloneFlags::CLONE_NEWPID;
    }
    if config.network {
        flags |= CloneFlags::CLONE_NEWNET;
    }
    if config.ipc {
        flags |= CloneFlags::CLONE_NEWIPC;
    }
    if config.uts {
        flags |= CloneFlags::CLONE_NEWUTS;
    }

    tracing::info!(?flags, "creating namespaces via unshare");
    unshare(flags).map_err(|e| ContainustError::PermissionDenied {
        message: format!("unshare failed: {e}"),
    })?;
    Ok(())
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — namespace creation requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn create_namespaces(_config: &NamespaceConfig) -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_config_default_has_all_flags_true() {
        let config = NamespaceConfig::default();
        assert!(config.pid);
        assert!(config.mount);
        assert!(config.network);
        assert!(config.user);
        assert!(config.ipc);
        assert!(config.uts);
    }

    #[test]
    fn namespace_config_clone_and_debug_derived() {
        let config = NamespaceConfig::default();
        let cloned = config.clone();
        assert_eq!(format!("{config:?}"), format!("{cloned:?}"));
    }

    #[test]
    fn namespace_config_all_false_can_be_constructed() {
        let config = NamespaceConfig {
            pid: false,
            mount: false,
            network: false,
            user: false,
            ipc: false,
            uts: false,
        };
        assert!(!config.pid);
        assert!(!config.mount);
        assert!(!config.network);
        assert!(!config.user);
        assert!(!config.ipc);
        assert!(!config.uts);
    }

    #[test]
    fn namespace_config_partial_selection() {
        let config = NamespaceConfig {
            pid: true,
            mount: true,
            network: false,
            user: false,
            ipc: false,
            uts: true,
        };
        assert!(config.pid);
        assert!(config.mount);
        assert!(!config.network);
        assert!(!config.user);
        assert!(!config.ipc);
        assert!(config.uts);
    }

    /// Requires root — ignored in CI. Tests that Linux syscall entry is reached.
    #[test]
    #[ignore = "requires root privileges"]
    fn create_namespaces_all_succeed_with_root() {
        let config = NamespaceConfig::default();
        let result = create_namespaces(&config);
        assert!(result.is_ok());
    }

    /// Tests that a config with no namespaces still succeeds (nothing to unshare).
    #[test]
    #[ignore = "requires root privileges"]
    fn create_namespaces_empty_config_succeeds_with_root() {
        let config = NamespaceConfig {
            pid: false,
            mount: false,
            network: false,
            user: false,
            ipc: false,
            uts: false,
        };
        let result = create_namespaces(&config);
        assert!(result.is_ok());
    }
}
