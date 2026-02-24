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
pub fn start_tracer(_target_pid: u32) -> Result<()> {
    tracing::info!(pid = _target_pid, "starting syscall tracer");
    Ok(())
}
