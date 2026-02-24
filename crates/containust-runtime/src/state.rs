//! Persistent state management.
//!
//! Maintains a local JSON index of all containers and their current
//! states, enabling daemon-less lifecycle management.

use std::path::Path;

use containust_common::error::{ContainustError, Result};
use containust_common::types::{ContainerId, ContainerState};
use serde::{Deserialize, Serialize};

/// Persistent record of a container's state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateEntry {
    /// Container identifier.
    pub id: ContainerId,
    /// Human-readable name.
    pub name: String,
    /// Current lifecycle state.
    pub state: ContainerState,
    /// PID of the init process (if running).
    pub pid: Option<u32>,
    /// Image source URI.
    pub image: String,
    /// Rootfs path on disk.
    pub rootfs_path: Option<String>,
    /// Log file path.
    pub log_path: Option<String>,
    /// ISO-8601 timestamp of creation.
    pub created_at: String,
}

/// Serializable collection of all container state entries.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StateFile {
    /// All tracked containers.
    pub containers: Vec<StateEntry>,
}

/// Loads the state index from disk.
///
/// Returns an empty `StateFile` if the file does not exist yet.
///
/// # Errors
///
/// Returns an error if the file exists but cannot be read or parsed.
pub fn load_state(path: &Path) -> Result<StateFile> {
    if !path.exists() {
        return Ok(StateFile::default());
    }
    let content = std::fs::read_to_string(path).map_err(|e| ContainustError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let state: StateFile = serde_json::from_str(&content)?;
    tracing::debug!(containers = state.containers.len(), "state loaded");
    Ok(state)
}

/// Persists the state index to disk.
///
/// Creates parent directories if they do not exist.
///
/// # Errors
///
/// Returns an error if the file cannot be written.
pub fn save_state(path: &Path, state: &StateFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ContainustError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    let json = serde_json::to_string_pretty(state)?;
    std::fs::write(path, json).map_err(|e| ContainustError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    tracing::debug!(path = %path.display(), "state saved");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_file_returns_empty_state() {
        let path = std::path::Path::new("/nonexistent/state.json");
        let state = load_state(path).expect("should return empty");
        assert!(state.containers.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.json");

        let state = StateFile {
            containers: vec![StateEntry {
                id: ContainerId::new("test-1"),
                name: "my-container".into(),
                state: ContainerState::Running,
                pid: Some(1234),
                image: "myapp:latest".into(),
                rootfs_path: Some("/var/lib/containust/rootfs/test-1".into()),
                log_path: None,
                created_at: "2026-01-01T00:00:00Z".into(),
            }],
        };

        save_state(&path, &state).expect("save should succeed");
        let loaded = load_state(&path).expect("load should succeed");

        assert_eq!(loaded.containers.len(), 1);
        assert_eq!(loaded.containers[0].id, ContainerId::new("test-1"));
        assert_eq!(loaded.containers[0].name, "my-container");
        assert_eq!(loaded.containers[0].state, ContainerState::Running);
        assert_eq!(loaded.containers[0].pid, Some(1234));
        assert_eq!(loaded.containers[0].image, "myapp:latest");
    }

    #[test]
    fn save_creates_parent_directories() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("nested").join("deep").join("state.json");

        let state = StateFile::default();
        save_state(&path, &state).expect("save should create dirs");
        assert!(path.exists());
    }

    #[test]
    fn load_empty_containers_list() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.json");

        let state = StateFile::default();
        save_state(&path, &state).expect("save");
        let loaded = load_state(&path).expect("load");
        assert!(loaded.containers.is_empty());
    }

    #[test]
    fn save_and_load_multiple_containers() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.json");

        let state = StateFile {
            containers: vec![
                StateEntry {
                    id: ContainerId::new("c1"),
                    name: "web".into(),
                    state: ContainerState::Running,
                    pid: Some(100),
                    image: "web:1.0".into(),
                    rootfs_path: None,
                    log_path: None,
                    created_at: "2026-01-01T00:00:00Z".into(),
                },
                StateEntry {
                    id: ContainerId::new("c2"),
                    name: "db".into(),
                    state: ContainerState::Stopped,
                    pid: None,
                    image: "postgres:15".into(),
                    rootfs_path: None,
                    log_path: None,
                    created_at: "2026-01-01T00:00:00Z".into(),
                },
            ],
        };

        save_state(&path, &state).expect("save");
        let loaded = load_state(&path).expect("load");
        assert_eq!(loaded.containers.len(), 2);
        assert_eq!(loaded.containers[0].name, "web");
        assert_eq!(loaded.containers[1].name, "db");
    }
}
