//! Persistent state management.
//!
//! Maintains a local JSON index of all containers and their current
//! states, enabling daemon-less lifecycle management.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use containust_common::constants::STATE_SCHEMA_VERSION;
use containust_common::error::{ContainustError, Result};
use containust_common::types::{ContainerId, ContainerState};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

/// Current on-disk state schema (alias of [`STATE_SCHEMA_VERSION`]).
pub const CURRENT_STATE_SCHEMA: u32 = STATE_SCHEMA_VERSION;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

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
    /// Command used to start the container.
    #[serde(default)]
    pub command: Vec<String>,
    /// Environment variables passed to the container process.
    #[serde(default)]
    pub env: Vec<(String, String)>,
    /// Configured memory limit in bytes.
    #[serde(default)]
    pub memory_bytes: Option<u64>,
    /// Configured CPU weight.
    #[serde(default)]
    pub cpu_shares: Option<u64>,
    /// Whether the root filesystem is read-only.
    #[serde(default = "default_readonly_rootfs")]
    pub readonly_rootfs: bool,
    /// Host-to-container bind mounts.
    #[serde(default)]
    pub volumes: Vec<String>,
    /// Rootfs path on disk.
    pub rootfs_path: Option<String>,
    /// Log file path.
    pub log_path: Option<String>,
    /// ISO-8601 timestamp of creation.
    pub created_at: String,
}

const fn default_readonly_rootfs() -> bool {
    true
}

/// Serializable collection of all container state entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateFile {
    /// Schema version used to serialize this state file.
    #[serde(default = "legacy_state_schema")]
    pub schema_version: u32,
    /// All tracked containers.
    pub containers: Vec<StateEntry>,
}

impl Default for StateFile {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_STATE_SCHEMA,
            containers: Vec::new(),
        }
    }
}

const fn legacy_state_schema() -> u32 {
    1
}

/// Locked state storage for one project.
#[derive(Debug, Clone)]
pub struct StateStore {
    path: PathBuf,
    lock_path: PathBuf,
}

impl StateStore {
    /// Creates a store for `path` without touching the filesystem.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        let lock_path = PathBuf::from(format!("{}.lock", path.display()));
        Self { path, lock_path }
    }

    /// Returns the state index path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads a consistent snapshot under a shared project lock.
    ///
    /// # Errors
    ///
    /// Returns an error if the lock or state file cannot be opened or parsed.
    pub fn read(&self) -> Result<StateFile> {
        let _guard = self.lock(false)?;
        load_state_unlocked(&self.path)
    }

    /// Replaces the complete state under an exclusive project lock.
    ///
    /// # Errors
    ///
    /// Returns an error if locking or the atomic write fails.
    pub fn write(&self, state: &StateFile) -> Result<()> {
        let _guard = self.lock(true)?;
        save_state_unlocked(&self.path, state)
    }

    /// Mutates state while holding an exclusive project lock.
    ///
    /// The updated state is persisted atomically only when `operation` succeeds.
    ///
    /// # Errors
    ///
    /// Returns an error from locking, loading, the operation, or persistence.
    pub fn update<T>(&self, operation: impl FnOnce(&mut StateFile) -> Result<T>) -> Result<T> {
        let _guard = self.lock(true)?;
        let mut state = load_state_unlocked(&self.path)?;
        let output = operation(&mut state)?;
        save_state_unlocked(&self.path, &state)?;
        Ok(output)
    }

    /// Mutates state and writes only when the operation reports a change.
    ///
    /// # Errors
    ///
    /// Returns an error from locking, loading, the operation, or persistence.
    pub fn update_if_changed<T>(
        &self,
        operation: impl FnOnce(&mut StateFile) -> Result<(T, bool)>,
    ) -> Result<T> {
        let _guard = self.lock(true)?;
        let mut state = load_state_unlocked(&self.path)?;
        let (output, changed) = operation(&mut state)?;
        if changed {
            save_state_unlocked(&self.path, &state)?;
        }
        Ok(output)
    }

    fn lock(&self, exclusive: bool) -> Result<StateLock> {
        if let Some(parent) = self.lock_path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ContainustError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&self.lock_path)
            .map_err(|source| ContainustError::Io {
                path: self.lock_path.clone(),
                source,
            })?;
        let lock_result = if exclusive {
            FileExt::lock_exclusive(&file)
        } else {
            FileExt::lock_shared(&file)
        };
        lock_result.map_err(|source| ContainustError::Io {
            path: self.lock_path.clone(),
            source,
        })?;
        Ok(StateLock { file })
    }
}

struct StateLock {
    file: std::fs::File,
}

impl Drop for StateLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

/// Loads the state index from disk.
///
/// Returns an empty `StateFile` if the file does not exist yet.
///
/// # Errors
///
/// Returns an error if the file exists but cannot be read or parsed.
pub fn load_state(path: &Path) -> Result<StateFile> {
    StateStore::new(path.to_path_buf()).read()
}

fn load_state_unlocked(path: &Path) -> Result<StateFile> {
    if !path.exists() {
        return Ok(StateFile::default());
    }
    let content = std::fs::read_to_string(path).map_err(|e| ContainustError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let mut state: StateFile = serde_json::from_str(&content)?;
    migrate_state(&mut state)?;
    tracing::debug!(containers = state.containers.len(), "state loaded");
    Ok(state)
}

fn migrate_state(state: &mut StateFile) -> Result<()> {
    if state.schema_version > CURRENT_STATE_SCHEMA {
        return Err(ContainustError::Config {
            message: format!(
                "state schema {} is newer than supported schema {CURRENT_STATE_SCHEMA}",
                state.schema_version
            ),
        });
    }
    if state.schema_version < CURRENT_STATE_SCHEMA {
        state.schema_version = CURRENT_STATE_SCHEMA;
    }
    Ok(())
}

/// Persists the state index to disk.
///
/// Creates parent directories if they do not exist.
///
/// # Errors
///
/// Returns an error if the file cannot be written.
pub fn save_state(path: &Path, state: &StateFile) -> Result<()> {
    StateStore::new(path.to_path_buf()).write(state)
}

fn save_state_unlocked(path: &Path, state: &StateFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ContainustError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    let mut persisted = state.clone();
    persisted.schema_version = CURRENT_STATE_SCHEMA;
    let json = serde_json::to_vec_pretty(&persisted)?;
    let temp_path = temporary_path(path);
    let write_result: Result<()> = (|| {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|source| ContainustError::Io {
                path: temp_path.clone(),
                source,
            })?;
        file.write_all(&json)
            .map_err(|source| ContainustError::Io {
                path: temp_path.clone(),
                source,
            })?;
        file.sync_all().map_err(|source| ContainustError::Io {
            path: temp_path.clone(),
            source,
        })?;
        drop(file);
        atomic_replace(&temp_path, path)?;
        #[cfg(unix)]
        sync_parent(path)?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    write_result?;
    tracing::debug!(path = %path.display(), "state saved");
    Ok(())
}

#[cfg(not(windows))]
fn atomic_replace(temp_path: &Path, path: &Path) -> Result<()> {
    std::fs::rename(temp_path, path).map_err(|source| ContainustError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn atomic_replace(temp_path: &Path, path: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{REPLACEFILE_WRITE_THROUGH, ReplaceFileW};

    if !path.exists() {
        return std::fs::rename(temp_path, path).map_err(|source| ContainustError::Io {
            path: path.to_path_buf(),
            source,
        });
    }

    let destination: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let replacement: Vec<u16> = temp_path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: both paths are NUL-terminated UTF-16 buffers that live through the call;
    // the optional backup, exclude, and reserved pointers are intentionally null.
    let replaced = unsafe {
        ReplaceFileW(
            destination.as_ptr(),
            replacement.as_ptr(),
            std::ptr::null(),
            REPLACEFILE_WRITE_THROUGH,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if replaced == 0 {
        return Err(ContainustError::Io {
            path: path.to_path_buf(),
            source: std::io::Error::last_os_error(),
        });
    }
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("state.json");
    path.with_file_name(format!(".{name}.{}.{}.tmp", std::process::id(), counter))
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    let directory = std::fs::File::open(parent).map_err(|source| ContainustError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    directory.sync_all().map_err(|source| ContainustError::Io {
        path: parent.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_entry(id: impl Into<String>) -> StateEntry {
        let id = id.into();
        StateEntry {
            id: ContainerId::new(&id),
            name: id,
            state: ContainerState::Stopped,
            pid: None,
            image: "file:///image".into(),
            command: Vec::new(),
            env: Vec::new(),
            memory_bytes: None,
            cpu_shares: None,
            readonly_rootfs: true,
            volumes: Vec::new(),
            rootfs_path: None,
            log_path: None,
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    fn append_test_entry(store: &StateStore, id: String) {
        store
            .update(|state| {
                state.containers.push(test_entry(id));
                Ok(())
            })
            .expect("locked update");
    }

    fn append_worker_entries(store: &StateStore, worker: &str, count: usize) {
        for item in 0..count {
            append_test_entry(store, format!("{worker}-{item}"));
        }
    }

    #[test]
    fn load_missing_file_returns_empty_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = load_state(&dir.path().join("missing.json")).expect("should return empty");
        assert!(state.containers.is_empty());
        assert_eq!(state.schema_version, CURRENT_STATE_SCHEMA);
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
                command: vec!["sh".into()],
                env: vec![("KEY".into(), "value".into())],
                memory_bytes: Some(128),
                cpu_shares: Some(512),
                readonly_rootfs: true,
                volumes: Vec::new(),
                rootfs_path: Some("/var/lib/containust/rootfs/test-1".into()),
                log_path: None,
                created_at: "2026-01-01T00:00:00Z".into(),
            }],
            ..StateFile::default()
        };

        save_state(&path, &state).expect("save should succeed");
        let loaded = load_state(&path).expect("load should succeed");

        assert_eq!(loaded.containers.len(), 1);
        assert_eq!(loaded.containers[0].id, ContainerId::new("test-1"));
        assert_eq!(loaded.containers[0].name, "my-container");
        assert_eq!(loaded.containers[0].state, ContainerState::Running);
        assert_eq!(loaded.containers[0].pid, Some(1234));
        assert_eq!(loaded.containers[0].image, "myapp:latest");
        assert_eq!(loaded.containers[0].command, vec!["sh"]);
        assert_eq!(
            loaded.containers[0].env,
            vec![("KEY".into(), "value".into())]
        );
        assert_eq!(loaded.containers[0].memory_bytes, Some(128));
        assert_eq!(loaded.containers[0].cpu_shares, Some(512));
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
                    command: Vec::new(),
                    env: Vec::new(),
                    memory_bytes: None,
                    cpu_shares: None,
                    readonly_rootfs: true,
                    volumes: Vec::new(),
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
                    command: Vec::new(),
                    env: Vec::new(),
                    memory_bytes: None,
                    cpu_shares: None,
                    readonly_rootfs: true,
                    volumes: Vec::new(),
                    rootfs_path: None,
                    log_path: None,
                    created_at: "2026-01-01T00:00:00Z".into(),
                },
            ],
            ..StateFile::default()
        };

        save_state(&path, &state).expect("save");
        let loaded = load_state(&path).expect("load");
        assert_eq!(loaded.containers.len(), 2);
        assert_eq!(loaded.containers[0].name, "web");
        assert_eq!(loaded.containers[1].name, "db");
    }

    #[test]
    fn save_replaces_existing_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.json");
        let first = StateFile {
            containers: vec![test_entry("first")],
            ..StateFile::default()
        };
        let second = StateFile {
            containers: vec![test_entry("second")],
            ..StateFile::default()
        };

        save_state(&path, &first).expect("save initial state");
        save_state(&path, &second).expect("replace existing state");

        let loaded = load_state(&path).expect("load replacement");
        assert_eq!(loaded.containers.len(), 1);
        assert_eq!(loaded.containers[0].id, ContainerId::new("second"));
    }

    #[test]
    fn legacy_state_is_migrated_to_current_schema() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.json");
        std::fs::write(&path, r#"{"containers":[]}"#).expect("legacy state");

        let migrated = load_state(&path).expect("migrate");
        assert_eq!(migrated.schema_version, CURRENT_STATE_SCHEMA);
        save_state(&path, &migrated).expect("persist migration");
        let persisted = std::fs::read_to_string(path).expect("read persisted");
        assert!(persisted.contains("\"schema_version\": 2"));
    }

    #[test]
    fn legacy_container_fields_receive_migration_defaults() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.json");
        std::fs::write(
            &path,
            r#"{
                "containers": [{
                    "id": "legacy",
                    "name": "legacy",
                    "state": "Stopped",
                    "pid": null,
                    "image": "file:///legacy",
                    "rootfs_path": null,
                    "log_path": null,
                    "created_at": "2025-01-01T00:00:00Z"
                }]
            }"#,
        )
        .expect("legacy state");

        let migrated = load_state(&path).expect("migrate");
        let entry = &migrated.containers[0];
        assert!(entry.command.is_empty());
        assert!(entry.env.is_empty());
        assert!(entry.readonly_rootfs);
        assert!(entry.volumes.is_empty());
    }

    #[test]
    fn future_state_schema_is_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.json");
        std::fs::write(&path, r#"{"schema_version":99,"containers":[]}"#).expect("future state");

        let error = load_state(&path).expect_err("future schema must fail");
        assert!(error.to_string().contains("newer than supported"));
    }

    #[test]
    fn interrupted_temporary_write_does_not_replace_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.json");
        let state = StateFile {
            containers: vec![test_entry("stable")],
            ..StateFile::default()
        };
        save_state(&path, &state).expect("save stable state");
        std::fs::write(dir.path().join(".state.json.interrupted.tmp"), b"{").expect("partial temp");

        let loaded = load_state(&path).expect("load stable state");
        assert_eq!(loaded.containers.len(), 1);
        assert_eq!(loaded.containers[0].name, "stable");
    }

    #[test]
    fn concurrent_updates_do_not_lose_entries() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = std::sync::Arc::new(StateStore::new(dir.path().join("state.json")));
        let mut workers = Vec::new();
        for worker in 0..4 {
            let store = std::sync::Arc::clone(&store);
            workers.push(std::thread::spawn(move || {
                append_worker_entries(&store, &format!("worker-{worker}"), 10);
            }));
        }
        for worker in workers {
            worker.join().expect("worker join");
        }

        let loaded = store.read().expect("read final state");
        assert_eq!(loaded.containers.len(), 40);
    }

    #[test]
    fn concurrent_process_updates_do_not_lose_entries() {
        const PATH_ENV: &str = "CONTAINUST_STATE_LOCK_TEST_PATH";
        const WORKER_ENV: &str = "CONTAINUST_STATE_LOCK_TEST_WORKER";

        if let (Some(path), Ok(worker)) = (std::env::var_os(PATH_ENV), std::env::var(WORKER_ENV)) {
            let store = StateStore::new(PathBuf::from(path));
            append_worker_entries(&store, &format!("process-{worker}"), 5);
            return;
        }

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.json");
        let executable = std::env::current_exe().expect("test executable");
        let mut children = Vec::new();
        for worker in 0..3 {
            children.push(
                std::process::Command::new(&executable)
                    .args([
                        "--exact",
                        "state::tests::concurrent_process_updates_do_not_lose_entries",
                    ])
                    .env(PATH_ENV, &path)
                    .env(WORKER_ENV, worker.to_string())
                    .spawn()
                    .expect("spawn worker"),
            );
        }
        for mut child in children {
            assert!(child.wait().expect("wait worker").success());
        }

        let state = load_state(&path).expect("read process state");
        assert_eq!(state.containers.len(), 15);
    }
}
