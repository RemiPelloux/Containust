//! Linux capability management for least-privilege execution.
//!
//! Drops all capabilities by default and only retains those
//! explicitly requested by the container configuration.

use containust_common::error::{ContainustError, Result};

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

#[cfg(target_os = "linux")]
impl Capability {
    /// Returns the Linux capability number for this capability.
    const fn linux_cap_number(self) -> u32 {
        match self {
            Self::Chown => 0,
            Self::Kill => 5,
            Self::Setgid => 6,
            Self::Setuid => 7,
            Self::NetBindService => 10,
        }
    }
}

/// Maximum capability number to iterate when dropping.
#[cfg(target_os = "linux")]
const CAP_LAST_CAP: u32 = 40;

/// Drops all Linux capabilities except those in the allowlist.
///
/// Iterates over all capability numbers 0..40 and drops each one
/// that is not in the `keep` set using `prctl(PR_CAPBSET_DROP)`.
///
/// # Errors
///
/// Returns an error if capability manipulation fails on a non-Linux platform.
#[cfg(target_os = "linux")]
pub fn drop_capabilities(keep: &[Capability]) -> Result<()> {
    let kept_caps: std::collections::HashSet<u32> =
        keep.iter().map(|c| c.linux_cap_number()).collect();

    for cap in 0..CAP_LAST_CAP {
        if kept_caps.contains(&cap) {
            continue;
        }
        drop_single_cap(cap)?;
    }
    tracing::info!(retained = keep.len(), "capabilities dropped");
    Ok(())
}

#[cfg(target_os = "linux")]
fn drop_single_cap(cap: u32) -> Result<()> {
    // SAFETY: prctl with PR_CAPBSET_DROP only removes capabilities from the
    // bounding set. Returns EINVAL for invalid capability numbers.
    let ret = unsafe { libc::prctl(libc::PR_CAPBSET_DROP, cap, 0, 0, 0) };
    if ret != -1 {
        return Ok(());
    }
    let errno = std::io::Error::last_os_error();
    if errno.raw_os_error() == Some(libc::EINVAL) {
        return Ok(());
    }
    Err(ContainustError::PermissionDenied {
        message: format!("failed to drop capability {cap}: {errno}"),
    })
}

/// Stub for non-Linux platforms.
///
/// # Errors
///
/// Always returns an error — capability management requires Linux.
#[cfg(not(target_os = "linux"))]
pub fn drop_capabilities(_keep: &[Capability]) -> Result<()> {
    Err(ContainustError::Config {
        message: "Linux required for native container operations".into(),
    })
}
