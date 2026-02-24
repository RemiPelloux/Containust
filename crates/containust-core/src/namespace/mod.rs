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
