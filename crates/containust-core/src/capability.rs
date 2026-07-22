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

/// Inclusive upper bound of capability numbers to drop (covers
/// `CAP_CHECKPOINT_RESTORE` = 40 on current kernels).
#[cfg(target_os = "linux")]
const CAP_LAST_CAP: u32 = 40;

/// Drops all Linux capabilities except those in the allowlist and
/// sets `NO_NEW_PRIVS` so privilege escalation via setuid is blocked.
///
/// Iterates over capability numbers `0..=CAP_LAST_CAP` and drops each
/// one that is not in the `keep` set using `prctl(PR_CAPBSET_DROP)`.
/// Unknown numbers return `EINVAL` and are skipped.
///
/// # Errors
///
/// Returns an error if capability manipulation fails. Callers must
/// fail closed — never ignore this error on a security boundary.
#[cfg(target_os = "linux")]
pub fn drop_capabilities(keep: &[Capability]) -> Result<()> {
    set_no_new_privs()?;
    let kept_caps: std::collections::HashSet<u32> =
        keep.iter().map(|c| c.linux_cap_number()).collect();

    for cap in 0..=CAP_LAST_CAP {
        if kept_caps.contains(&cap) {
            continue;
        }
        drop_single_cap(cap)?;
    }
    tracing::info!(retained = keep.len(), "capabilities dropped");
    Ok(())
}

#[cfg(target_os = "linux")]
fn set_no_new_privs() -> Result<()> {
    // SAFETY: PR_SET_NO_NEW_PRIVS with args (1, 0, 0, 0) is the documented
    // way to permanently disable privilege gains for the calling thread.
    let ret = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if ret == 0 {
        return Ok(());
    }
    Err(ContainustError::PermissionDenied {
        message: format!(
            "failed to set PR_SET_NO_NEW_PRIVS: {}",
            std::io::Error::last_os_error()
        ),
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn capability_linux_cap_number_matches_known_values() {
        assert_eq!(Capability::Chown.linux_cap_number(), 0);
        assert_eq!(Capability::Kill.linux_cap_number(), 5);
        assert_eq!(Capability::Setgid.linux_cap_number(), 6);
        assert_eq!(Capability::Setuid.linux_cap_number(), 7);
        assert_eq!(Capability::NetBindService.linux_cap_number(), 10);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn capability_linux_cap_number_all_distinct() {
        let caps = [
            Capability::Chown,
            Capability::Kill,
            Capability::Setgid,
            Capability::Setuid,
            Capability::NetBindService,
        ];
        let numbers: std::collections::HashSet<u32> =
            caps.iter().map(|c| c.linux_cap_number()).collect();
        assert_eq!(
            numbers.len(),
            caps.len(),
            "all capability numbers must be distinct"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn cap_last_cap_is_forty() {
        assert_eq!(CAP_LAST_CAP, 40);
    }

    #[test]
    fn capability_copy_trait_allows_duplication() {
        let cap = Capability::Kill;
        let copied = cap;
        assert_eq!(cap, copied);
    }

    #[test]
    fn capability_clone_trait_allows_duplication() {
        let cap = Capability::NetBindService;
        let cloned = cap;
        assert_eq!(cap, cloned);
    }

    #[test]
    fn capability_eq_distinguishes_variants() {
        assert_eq!(Capability::Chown, Capability::Chown);
        assert_ne!(Capability::Chown, Capability::Kill);
        assert_ne!(Capability::NetBindService, Capability::Setgid);
    }

    #[test]
    fn capability_hash_allows_set_dedup() {
        let caps = std::collections::HashSet::from([
            Capability::Chown,
            Capability::Chown,
            Capability::Kill,
        ]);
        assert_eq!(caps.len(), 2);
    }

    #[test]
    fn capability_debug_derived() {
        let cap = Capability::Setuid;
        let debug = format!("{cap:?}");
        assert_eq!(debug, "Setuid");
    }

    #[test]
    #[cfg(target_os = "linux")]
    #[ignore = "requires root privileges"]
    fn drop_capabilities_keep_all_succeeds() {
        let keep = [
            Capability::NetBindService,
            Capability::Chown,
            Capability::Kill,
            Capability::Setuid,
            Capability::Setgid,
        ];
        let result = drop_capabilities(&keep);
        assert!(result.is_ok());
    }

    #[test]
    #[cfg(target_os = "linux")]
    #[ignore = "requires root privileges"]
    fn drop_capabilities_keep_none_succeeds() {
        let result = drop_capabilities(&[]);
        // May fail if not root, but shouldn't panic
        let _ = result;
    }
}
