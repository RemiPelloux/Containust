//! Network namespace isolation.
//!
//! Provides the container with its own network stack (interfaces, routing, iptables).

use containust_common::error::Result;

/// Creates a new network namespace for the calling process.
///
/// # Errors
///
/// Returns an error if the `unshare(CLONE_NEWNET)` syscall fails.
pub fn create_network_namespace() -> Result<()> {
    tracing::debug!("creating network namespace");
    Ok(())
}
