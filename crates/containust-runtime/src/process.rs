//! Process spawning inside isolated namespaces.
//!
//! Forks a child process, enters the configured namespaces, applies
//! cgroup limits, and executes the target command.

use containust_common::error::Result;

/// Spawns a new process inside the container's namespaces.
///
/// # Errors
///
/// Returns an error if fork, namespace entry, or exec fails.
pub fn spawn_container_process(
    _command: &[String],
    _env: &[(String, String)],
    _rootfs: &std::path::Path,
) -> Result<u32> {
    tracing::info!("spawning container process");
    todo!()
}
