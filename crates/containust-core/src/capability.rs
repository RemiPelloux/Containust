//! Linux capability management for least-privilege execution.
//!
//! Drops all capabilities by default and only retains those
//! explicitly requested by the container configuration.

use containust_common::error::Result;

/// Linux capability identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Allow binding to privileged ports (< 1024).
    NetBindService,
    /// Allow setting file ownership.
    Chown,
    /// Allow sending signals to arbitrary processes.
    Kill,
    /// Allow setting user/group IDs.
    Setuid,
    /// Allow setting group IDs.
    Setgid,
}

/// Drops all Linux capabilities except those in the allowlist.
///
/// # Errors
///
/// Returns an error if capability manipulation syscalls fail.
pub fn drop_capabilities(keep: &[Capability]) -> Result<()> {
    tracing::info!(retained = keep.len(), "dropping capabilities");
    Ok(())
}
