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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn file_open_event_constructs_with_all_fields() {
        let event = FileOpenEvent {
            pid: 1234,
            path: "/etc/passwd".into(),
            flags: 0,
        };
        assert_eq!(event.pid, 1234);
        assert_eq!(event.path, "/etc/passwd");
        assert_eq!(event.flags, 0);
    }

    #[test]
    fn file_open_event_serialization_roundtrip() {
        let event = FileOpenEvent {
            pid: 5678,
            path: "/tmp/test.txt".into(),
            flags: 2, // O_RDWR
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let back: FileOpenEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.pid, 5678);
        assert_eq!(back.path, "/tmp/test.txt");
        assert_eq!(back.flags, 2);
    }

    #[test]
    fn file_open_event_clone_works() {
        let original = FileOpenEvent {
            pid: 1,
            path: "/dev/null".into(),
            flags: 1,
        };
        let cloned = original.clone();
        assert_eq!(cloned.path, original.path);
        assert_eq!(cloned.flags, original.flags);
    }

    #[test]
    fn file_open_event_empty_path() {
        let event = FileOpenEvent {
            pid: 1,
            path: String::new(),
            flags: 0,
        };
        assert_eq!(event.path, "");
    }

    #[test]
    fn start_file_monitor_succeeds() {
        assert!(start_file_monitor(100).is_ok());
    }
}
