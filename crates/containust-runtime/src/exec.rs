//! Namespace joining for executing commands in running containers.
//!
//! Implements `ctst exec` by joining the target container's namespaces
//! and spawning a new process inside them.

use containust_common::error::Result;
use containust_common::types::ContainerId;

/// Joins the namespaces of a running container and executes a command.
///
/// # Errors
///
/// Returns an error if the container is not running or namespace joining fails.
pub fn exec_in_container(container_id: &ContainerId, _command: &[String]) -> Result<()> {
    tracing::info!(id = %container_id, "exec into container");
    todo!()
}
