//! Syscall tracing via eBPF.
//!
//! Attaches to tracepoints to monitor system calls made by
//! container processes in real time.

use containust_common::error::Result;
use serde::{Deserialize, Serialize};

#[cfg(all(target_os = "linux", feature = "ebpf"))]
use crate::lifecycle::{ProbeAvailability, probe_availability};

/// A captured syscall event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyscallEvent {
    /// PID of the process that made the syscall.
    pub pid: u32,
    /// Syscall number.
    pub syscall_nr: u64,
    /// Timestamp in nanoseconds.
    pub timestamp_ns: u64,
}

/// Starts the syscall tracer for a specific container PID namespace.
///
/// Prefer [`crate::lifecycle::attach`] for the full attach/detach API.
///
/// # Errors
///
/// Returns an error when probes are unavailable on this build/host.
pub fn start_tracer(target_pid: u32) -> Result<()> {
    crate::lifecycle::attach(target_pid)
}

/// Low-level attach used when availability has already been checked.
#[cfg(all(target_os = "linux", feature = "ebpf"))]
pub(crate) fn start_tracer_unchecked(target_pid: u32) {
    debug_assert!(matches!(probe_availability(), ProbeAvailability::Available));
    tracing::info!(pid = target_pid, "starting syscall tracer");
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::lifecycle::{ProbeAvailability, probe_availability};

    #[test]
    fn syscall_event_constructs_with_all_fields() {
        let event = SyscallEvent {
            pid: 1234,
            syscall_nr: 60,
            timestamp_ns: 1_700_000_000_000_000,
        };
        assert_eq!(event.pid, 1234);
    }

    #[test]
    fn syscall_event_serialization_roundtrip() {
        let event = SyscallEvent {
            pid: 42,
            syscall_nr: 1,
            timestamp_ns: 100_000_000,
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let back: SyscallEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.pid, 42);
    }

    #[test]
    fn start_tracer_respects_availability() {
        match probe_availability() {
            ProbeAvailability::Available => assert!(start_tracer(100).is_ok()),
            ProbeAvailability::FeatureDisabled | ProbeAvailability::UnsupportedOs => {
                assert!(start_tracer(100).is_err());
            }
        }
    }
}
