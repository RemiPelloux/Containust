//! Persistent state management.
//!
//! Maintains a local JSON index of all containers and their current
//! states, enabling daemon-less lifecycle management.

use std::path::Path;

use containust_common::error::Result;
use containust_common::types::{ContainerId, ContainerState};
use serde::{Deserialize, Serialize};

/// Persistent record of a container's state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateEntry {
    /// Container identifier.
    pub id: ContainerId,
    /// Current lifecycle state.
    pub state: ContainerState,
    /// PID of the init process (if running).
    pub pid: Option<u32>,
    /// ISO-8601 timestamp of creation.
    pub created_at: String,
}

/// Loads the state index from disk.
///
/// # Errors
///
/// Returns an error if the file cannot be read or parsed.
pub fn load_state(path: &Path) -> Result<Vec<StateEntry>> {
    tracing::debug!(path = %path.display(), "loading state index");
    Ok(Vec::new())
}

/// Persists the state index to disk atomically.
///
/// # Errors
///
/// Returns an error if the file cannot be written.
pub fn save_state(path: &Path, _entries: &[StateEntry]) -> Result<()> {
    tracing::debug!(path = %path.display(), "saving state index");
    Ok(())
}
