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

use containust_common::error::Result;

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
/// # Errors
///
/// Returns an error if any namespace creation syscall fails.
pub fn create_namespaces(config: &NamespaceConfig) -> Result<()> {
    tracing::info!(config = ?config, "creating namespaces");
    // Implementation will call unshare(2) with the appropriate flags
    Ok(())
}
