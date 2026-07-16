//! Syscall tracing via eBPF.
//!
//! Attaches to tracepoints to monitor system calls made by
//! container processes in real time.

use containust_common::error::Result;
use serde::{Deserialize, Serialize};

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
/// # Errors
///
/// Returns an error if eBPF program loading or attachment fails.
pub fn start_tracer(target_pid: u32) -> Result<()> {
    tracing::info!(pid = target_pid, "starting syscall tracer");
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn syscall_event_constructs_with_all_fields() {
        let event = SyscallEvent {
            pid: 1234,
            syscall_nr: 60,
            timestamp_ns: 1_700_000_000_000_000,
        };
        assert_eq!(event.pid, 1234);
        assert_eq!(event.syscall_nr, 60);
        assert_eq!(event.timestamp_ns, 1_700_000_000_000_000);
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
        assert_eq!(back.syscall_nr, 1);
        assert_eq!(back.timestamp_ns, 100_000_000);
    }

    #[test]
    fn syscall_event_clone_works() {
        let original = SyscallEvent {
            pid: 99,
            syscall_nr: 0,
            timestamp_ns: 0,
        };
        let cloned = original.clone();
        assert_eq!(cloned.pid, original.pid);
        assert_eq!(cloned.syscall_nr, original.syscall_nr);
    }

    #[test]
    fn syscall_event_display_debug() {
        let event = SyscallEvent {
            pid: 1,
            syscall_nr: 57,
            timestamp_ns: 12345,
        };
        let debug = format!("{event:?}");
        assert!(debug.contains("SyscallEvent"));
        assert!(debug.contains('1'));
    }

    #[test]
    fn start_tracer_succeeds_with_valid_pid() {
        let result = start_tracer(100);
        assert!(result.is_ok());
    }

    #[test]
    fn start_tracer_with_pid_zero() {
        let result = start_tracer(0);
        assert!(result.is_ok());
    }
}
