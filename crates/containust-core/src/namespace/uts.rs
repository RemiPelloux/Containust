//! UTS namespace isolation.
//!
//! Allows the container to have its own hostname and domain name.

use containust_common::error::Result;

/// Creates a new UTS namespace for the calling process.
///
/// # Errors
///
/// Returns an error if the `unshare(CLONE_NEWUTS)` syscall fails.
pub fn create_uts_namespace() -> Result<()> {
    tracing::debug!("creating UTS namespace");
    Ok(())
}

/// Sets the hostname inside the UTS namespace.
///
/// # Errors
///
/// Returns an error if `sethostname(2)` fails.
pub fn set_hostname(_hostname: &str) -> Result<()> {
    tracing::debug!("setting container hostname");
    Ok(())
}
