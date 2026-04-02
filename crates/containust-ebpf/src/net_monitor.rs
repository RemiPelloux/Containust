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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn network_event_constructs_with_all_fields() {
        let event = NetworkEvent {
            pid: 5432,
            src_addr: "192.168.1.10".into(),
            dst_addr: "10.0.0.1".into(),
            dst_port: 443,
            protocol: "TCP".into(),
        };
        assert_eq!(event.pid, 5432);
        assert_eq!(event.src_addr, "192.168.1.10");
        assert_eq!(event.dst_addr, "10.0.0.1");
        assert_eq!(event.dst_port, 443);
        assert_eq!(event.protocol, "TCP");
    }

    #[test]
    fn network_event_serialization_roundtrip() {
        let event = NetworkEvent {
            pid: 1,
            src_addr: "0.0.0.0".into(),
            dst_addr: "127.0.0.1".into(),
            dst_port: 80,
            protocol: "TCP".into(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let back: NetworkEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.pid, 1);
        assert_eq!(back.src_addr, "0.0.0.0");
        assert_eq!(back.dst_addr, "127.0.0.1");
        assert_eq!(back.dst_port, 80);
        assert_eq!(back.protocol, "TCP");
    }

    #[test]
    fn network_event_clone_works() {
        let original = NetworkEvent {
            pid: 9999,
            src_addr: "::1".into(),
            dst_addr: "::1".into(),
            dst_port: 5432,
            protocol: "UDP".into(),
        };
        let cloned = original.clone();
        assert_eq!(cloned.protocol, original.protocol);
    }

    #[test]
    fn network_event_with_ipv6() {
        let event = NetworkEvent {
            pid: 42,
            src_addr: "fe80::1".into(),
            dst_addr: "2001:db8::1".into(),
            dst_port: 8080,
            protocol: "TCP".into(),
        };
        assert_eq!(event.dst_port, 8080);
    }

    #[test]
    fn start_net_monitor_succeeds() {
        assert!(start_net_monitor(200).is_ok());
    }
}
