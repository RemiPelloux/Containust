//! File open monitoring via eBPF.
//!
//! Tracks file open operations inside containers to detect
//! unexpected filesystem access.

use containust_common::error::Result;
use serde::{Deserialize, Serialize};

/// A captured file open event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOpenEvent {
    /// PID of the process.
    pub pid: u32,
    /// Path that was opened.
    pub path: String,
    /// Open flags.
    pub flags: u32,
}

/// Starts file open monitoring for a container.
///
/// # Errors
///
/// Returns an error if the eBPF program cannot be loaded.
pub fn start_file_monitor(target_pid: u32) -> Result<()> {
    tracing::info!(pid = target_pid, "starting file monitor");
    Ok(())
}
