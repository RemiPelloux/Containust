//! Probe attach/detach lifecycle with graceful degradation.

use containust_common::error::{ContainustError, Result};

/// Whether eBPF probes can run on this build/host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeAvailability {
    /// Feature enabled and host looks usable.
    Available,
    /// Built without the `ebpf` cargo feature.
    FeatureDisabled,
    /// Not a Linux host.
    UnsupportedOs,
}

/// Reports whether eBPF probes are usable in this binary/host combination.
#[must_use]
pub const fn probe_availability() -> ProbeAvailability {
    #[cfg(not(target_os = "linux"))]
    {
        ProbeAvailability::UnsupportedOs
    }
    #[cfg(all(target_os = "linux", not(feature = "ebpf")))]
    {
        ProbeAvailability::FeatureDisabled
    }
    #[cfg(all(target_os = "linux", feature = "ebpf"))]
    {
        ProbeAvailability::Available
    }
}

/// Human-readable status for `ctst doctor` and CLI messaging.
#[must_use]
pub const fn availability_message() -> &'static str {
    match probe_availability() {
        ProbeAvailability::Available => "available",
        ProbeAvailability::FeatureDisabled => {
            "not built (enable the containust-ebpf/ebpf feature on Linux)"
        }
        ProbeAvailability::UnsupportedOs => "unavailable on this OS (Linux only)",
    }
}

/// Attaches observability probes for `target_pid`.
///
/// # Errors
///
/// Returns a configuration error when probes are unavailable, or a domain
/// error when attach fails under the `ebpf` feature.
pub fn attach(target_pid: u32) -> Result<()> {
    match probe_availability() {
        ProbeAvailability::Available => attach_impl(target_pid),
        ProbeAvailability::FeatureDisabled => Err(ContainustError::Config {
            message: format!(
                "eBPF probes not built into this binary (pid {target_pid}); \
                 rebuild with --features ebpf on Linux"
            ),
        }),
        ProbeAvailability::UnsupportedOs => Err(ContainustError::Config {
            message: format!(
                "eBPF probes require Linux (requested pid {target_pid}); \
                 observability degrades gracefully on this platform"
            ),
        }),
    }
}

/// Detaches observability probes for `target_pid`.
///
/// # Errors
///
/// Returns a configuration error when probes are unavailable.
pub fn detach(target_pid: u32) -> Result<()> {
    match probe_availability() {
        ProbeAvailability::Available => detach_impl(target_pid),
        ProbeAvailability::FeatureDisabled | ProbeAvailability::UnsupportedOs => {
            tracing::debug!(
                pid = target_pid,
                status = availability_message(),
                "eBPF detach skipped"
            );
            Ok(())
        }
    }
}

#[cfg(all(target_os = "linux", feature = "ebpf"))]
fn attach_impl(target_pid: u32) -> Result<()> {
    crate::tracer::start_tracer_unchecked(target_pid);
    tracing::info!(pid = target_pid, "eBPF probes attached");
    Ok(())
}

/// Stub detach keeps `Result` so a real unload path can return errors later.
#[cfg(all(target_os = "linux", feature = "ebpf"))]
#[allow(clippy::unnecessary_wraps)]
fn detach_impl(target_pid: u32) -> Result<()> {
    tracing::info!(pid = target_pid, "eBPF probes detached");
    Ok(())
}

#[cfg(not(all(target_os = "linux", feature = "ebpf")))]
fn attach_impl(_target_pid: u32) -> Result<()> {
    Err(ContainustError::Config {
        message: "eBPF attach unreachable without linux+ebpf".into(),
    })
}

/// Stub detach keeps `Result` so a real unload path can return errors later.
#[cfg(not(all(target_os = "linux", feature = "ebpf")))]
#[allow(clippy::unnecessary_wraps, clippy::missing_const_for_fn)]
fn detach_impl(_target_pid: u32) -> Result<()> {
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn availability_message_is_non_empty() {
        assert!(!availability_message().is_empty());
    }

    #[test]
    fn attach_fails_closed_when_unavailable() {
        if matches!(probe_availability(), ProbeAvailability::Available) {
            return;
        }
        let err = attach(1).expect_err("unavailable");
        assert!(err.to_string().contains("eBPF") || err.to_string().contains("Linux"));
    }

    #[test]
    fn detach_is_idempotent_when_unavailable() {
        if matches!(probe_availability(), ProbeAvailability::Available) {
            return;
        }
        assert!(detach(1).is_ok());
    }
}
