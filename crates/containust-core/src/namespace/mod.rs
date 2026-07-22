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
    /// Secure defaults for the Linux spawn path (`unshare` + `exec`).
    ///
    /// Mount, network, IPC, and UTS are enabled. PID and user namespaces
    /// require additional machinery (double-fork / uid maps) and must be
    /// opted into once those paths are implemented — requesting them
    /// today fails closed via [`Self::validate_for_spawn`].
    fn default() -> Self {
        Self {
            pid: false,
            mount: true,
            network: true,
            user: false,
            ipc: true,
            uts: true,
        }
    }
}

impl NamespaceConfig {
    /// Validates that this configuration can be applied by the current
    /// Linux spawn path. Unsupported combinations fail closed.
    ///
    /// # Errors
    ///
    /// Returns an error when mount isolation is disabled, or when PID /
    /// user namespaces are requested before the runtime supports them.
    pub fn validate_for_spawn(&self) -> Result<()> {
        if !self.mount {
            return Err(ContainustError::Config {
                message: "mount namespace is required for Linux containers \
                          (pivot_root depends on it)"
                    .into(),
            });
        }
        if self.pid {
            return Err(ContainustError::Config {
                message: "PID namespace isolation is not yet supported by the \
                          spawn path (requires double-fork); leave namespaces.pid = false"
                    .into(),
            });
        }
        if self.user {
            return Err(ContainustError::Config {
                message: "user namespace isolation is not yet supported \
                          (requires uid/gid mapping); leave namespaces.user = false"
                    .into(),
            });
        }
        Ok(())
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
    fn namespace_config_default_enables_supported_isolation() {
        let config = NamespaceConfig::default();
        assert!(!config.pid);
        assert!(config.mount);
        assert!(config.network);
        assert!(!config.user);
        assert!(config.ipc);
        assert!(config.uts);
        assert!(config.validate_for_spawn().is_ok());
    }

    #[test]
    fn namespace_config_rejects_disabled_mount() {
        let config = NamespaceConfig {
            mount: false,
            ..NamespaceConfig::default()
        };
        assert!(config.validate_for_spawn().is_err());
    }

    #[test]
    fn namespace_config_rejects_unsupported_pid_and_user() {
        let with_pid = NamespaceConfig {
            pid: true,
            ..NamespaceConfig::default()
        };
        assert!(with_pid.validate_for_spawn().is_err());
        let with_user = NamespaceConfig {
            user: true,
            ..NamespaceConfig::default()
        };
        assert!(with_user.validate_for_spawn().is_err());
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
        let config = NamespaceConfig {
            pid: true,
            mount: true,
            network: true,
            user: true,
            ipc: true,
            uts: true,
        };
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
