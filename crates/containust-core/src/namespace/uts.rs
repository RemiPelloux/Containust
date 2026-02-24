//! UTS namespace isolation.
//!
//! Allows the container to have its own hostname and domain name.

use containust_common::error::{ContainustError, Result};

/// Creates a new UTS namespace for the calling process.
///
/// # Errors
///
/// Returns an error if the `unshare(CLONE_NEWUTS)` syscall fails.
#[cfg(target_os = "linux")]
pub fn create_uts_namespace() -> Result<()> {
    use nix::sched::{CloneFlags, unshare};

    unshare(CloneFlags::CLONE_NEWUTS).map_err(|e| ContainustError::PermissionDenied {
        message: format!("UTS namespace creation failed: {e}"),
    })?;
    tracing::debug!("UTS namespace created");
    Ok(())
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — UTS namespace requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn create_uts_namespace() -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}

/// Sets the hostname inside the UTS namespace.
///
/// # Errors
///
/// Returns an error if `sethostname(2)` fails.
#[cfg(target_os = "linux")]
pub fn set_hostname(hostname: &str) -> Result<()> {
    nix::unistd::sethostname(hostname).map_err(|e| ContainustError::PermissionDenied {
        message: format!("sethostname failed: {e}"),
    })?;
    tracing::debug!(hostname, "hostname set");
    Ok(())
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — hostname setting requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn set_hostname(_hostname: &str) -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}
