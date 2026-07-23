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
    /// Secure defaults for the Linux spawn path.
    ///
    /// Mount, network, IPC, and UTS are enabled. User and PID namespaces
    /// are off by default; the Linux backend opts into them via
    /// [`Self::with_user_and_pid`] once the pipe-sync spawn path is used.
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
    /// Enables user and PID namespaces on top of the secure defaults.
    #[must_use]
    pub const fn with_user_and_pid(mut self) -> Self {
        self.user = true;
        self.pid = true;
        self
    }

    /// Validates that this configuration can be applied by the Linux spawn path.
    ///
    /// # Errors
    ///
    /// Returns an error when mount isolation is disabled (`pivot_root` requires it).
    pub fn validate_for_spawn(&self) -> Result<()> {
        if !self.mount {
            return Err(ContainustError::Config {
                message: "mount namespace is required for Linux containers \
                          (pivot_root depends on it)"
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
    fn namespace_config_allows_user_and_pid_when_mount_enabled() {
        let config = NamespaceConfig::default().with_user_and_pid();
        assert!(config.user);
        assert!(config.pid);
        assert!(config.validate_for_spawn().is_ok());
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

    /// Requires root. Runs in a forked child because `CLONE_NEWUSER`
    /// requires a single-threaded process and the test harness is not one.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires root privileges"]
    fn create_namespaces_all_succeed_with_root() {
        let ok = crate::testutil::forked_probe_succeeds(|| {
            let config = NamespaceConfig {
                pid: true,
                mount: true,
                network: true,
                user: true,
                ipc: true,
                uts: true,
            };
            create_namespaces(&config).is_ok()
        });
        assert!(ok, "unshare of all namespaces failed in forked child");
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
