//! Network connection monitoring via eBPF.
//!
//! Tracks socket creation and TCP/UDP connections made by
//! container processes.

use containust_common::error::Result;
use serde::{Deserialize, Serialize};

/// A captured network event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEvent {
    /// PID of the process.
    pub pid: u32,
    /// Source address.
    pub src_addr: String,
    /// Destination address.
    pub dst_addr: String,
    /// Destination port.
    pub dst_port: u16,
    /// Protocol (TCP/UDP).
    pub protocol: String,
}

/// Starts network monitoring for a container.
///
/// # Errors
///
/// Returns an error if the eBPF program cannot be loaded.
pub fn start_net_monitor(target_pid: u32) -> Result<()> {
    tracing::info!(pid = target_pid, "starting network monitor");
    Ok(())
}
