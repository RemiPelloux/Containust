//! Linux native container backend using direct syscalls.

use std::path::PathBuf;

use containust_common::error::{ContainustError, Result};
use containust_common::types::ContainerId;

use super::{ContainerBackend, ContainerConfig, ContainerInfo};
use crate::exec::ExecOutput;

/// Backend that uses Linux kernel features directly.
///
/// Manages container state on disk and delegates process operations
/// to Linux namespaces, cgroups v2, `OverlayFS`, and `pivot_root`.
pub struct LinuxNativeBackend {
    data_dir: PathBuf,
}

impl LinuxNativeBackend {
    /// Creates a new Linux native backend.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data_dir: containust_common::constants::data_dir().clone(),
        }
    }
}

impl Default for LinuxNativeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ContainerBackend for LinuxNativeBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn create(&self, config: &ContainerConfig) -> Result<ContainerId> {
        let id = ContainerId::generate();
        tracing::info!(id = %id, name = %config.name, "creating container (Linux native)");

        // Prepare rootfs from the image source
        let rootfs = prepare_rootfs(&config.image, &id)?;

        let state_path = self.data_dir.join("state.json");
        let mut state = crate::state::load_state(&state_path)?;
        state.containers.push(crate::state::StateEntry {
            id: id.clone(),
            name: config.name.clone(),
            state: containust_common::types::ContainerState::Created,
            pid: None,
            image: config.image.clone(),
            command: config.command.clone(),
            env: config.env.clone(),
            memory_bytes: config.memory_bytes,
            cpu_shares: config.cpu_shares,
            rootfs_path: Some(rootfs.to_string_lossy().to_string()),
            log_path: Some(
                self.data_dir
                    .join("logs")
                    .join(format!("{id}.log"))
                    .to_string_lossy()
                    .to_string(),
            ),
            created_at: chrono::Utc::now().to_rfc3339(),
        });
        crate::state::save_state(&state_path, &state)?;
        tracing::info!(rootfs = %rootfs.display(), "rootfs prepared");
        Ok(id)
    }

    fn start(&self, id: &ContainerId) -> Result<u32> {
        tracing::info!(id = %id, "starting container (Linux native)");

        let state_path = self.data_dir.join("state.json");

        // Load state and look up the container entry
        let mut state = crate::state::load_state(&state_path)?;
        let idx = state
            .containers
            .iter()
            .position(|e| e.id == *id)
            .ok_or_else(|| ContainustError::NotFound {
                kind: "container",
                id: id.as_str().to_string(),
            })?;

        let entry = &state.containers[idx];
        if entry.state == containust_common::types::ContainerState::Running {
            return Err(ContainustError::Config {
                message: format!("container {id} is already running"),
            });
        }

        // Clone out the data we need so we can release the borrow
        let image = entry.image.clone();
        let existing_rootfs = entry.rootfs_path.clone();
        let stored_command = entry.command.clone();
        let env = entry.env.clone();
        let memory_bytes = entry.memory_bytes;
        let cpu_shares = entry.cpu_shares;
        let rootfs = match existing_rootfs {
            Some(p) => PathBuf::from(p),
            None => prepare_rootfs(&image, id)?,
        };

        // If we created a new rootfs, record it in state
        if state.containers[idx].rootfs_path.is_none() {
            state.containers[idx].rootfs_path = Some(rootfs.to_string_lossy().to_string());
            crate::state::save_state(&state_path, &state)?;
        }

        let command = if stored_command.is_empty() {
            derive_command_from_image(&image)
        } else {
            stored_command
        };

        // Spawn the container process with full namespace isolation
        let pid = crate::process::spawn_container_process(&command, &env, &rootfs)?;

        apply_cgroup_limits(id, pid, memory_bytes, cpu_shares);

        // Update state to Running
        let entry = &mut state.containers[idx];
        entry.state = containust_common::types::ContainerState::Running;
        entry.pid = Some(pid);
        crate::state::save_state(&state_path, &state)?;

        tracing::info!(pid, "container started (Linux native)");
        Ok(pid)
    }

    fn stop(&self, id: &ContainerId) -> Result<()> {
        tracing::info!(id = %id, "stopping container (Linux native)");

        let state_path = self.data_dir.join("state.json");
        let mut state = crate::state::load_state(&state_path)?;

        // If running, send SIGTERM then SIGKILL
        let entry = state
            .containers
            .iter_mut()
            .find(|e| e.id == *id)
            .ok_or_else(|| ContainustError::NotFound {
                kind: "container",
                id: id.to_string(),
            })?;
        let is_running = entry.state == containust_common::types::ContainerState::Running;
        if let Some(pid) = entry.pid.filter(|_| is_running) {
            terminate_process(pid);
        }
        entry.state = containust_common::types::ContainerState::Stopped;
        entry.pid = None;
        crate::state::save_state(&state_path, &state)?;
        cleanup_cgroup(id);

        Ok(())
    }

    fn exec(&self, id: &ContainerId, cmd: &[String]) -> Result<ExecOutput> {
        let state_path = self.data_dir.join("state.json");
        let state = crate::state::load_state(&state_path)?;
        let entry = state
            .containers
            .iter()
            .find(|e| e.id == *id)
            .ok_or_else(|| ContainustError::NotFound {
                kind: "container",
                id: id.to_string(),
            })?;
        let pid = entry.pid.ok_or_else(|| ContainustError::Config {
            message: format!("container {id} is not running"),
        })?;
        crate::exec::exec_in_container(id, pid, cmd)
    }

    fn remove(&self, id: &ContainerId) -> Result<()> {
        let state_path = self.data_dir.join("state.json");
        let mut state = crate::state::load_state(&state_path)?;

        // Clean up cgroup
        cleanup_cgroup(id);

        let before = state.containers.len();
        state.containers.retain(|e| e.id != *id);
        if state.containers.len() == before {
            return Err(ContainustError::NotFound {
                kind: "container",
                id: id.to_string(),
            });
        }
        crate::state::save_state(&state_path, &state)?;
        Ok(())
    }

    fn logs(&self, id: &ContainerId) -> Result<String> {
        crate::logs::read_logs(&self.data_dir, id.as_str())
    }

    fn list(&self) -> Result<Vec<ContainerInfo>> {
        let state_path = self.data_dir.join("state.json");
        let state = crate::state::load_state(&state_path)?;
        Ok(state
            .containers
            .iter()
            .map(|e| ContainerInfo {
                id: e.id.clone(),
                name: e.name.clone(),
                state: e.state.to_string(),
                pid: e.pid,
                image: e.image.clone(),
                created_at: e.created_at.clone(),
            })
            .collect())
    }

    fn is_available(&self) -> bool {
        cfg!(target_os = "linux")
    }
}

/// Sends SIGTERM followed by SIGKILL after a 2-second grace period.
fn terminate_process(pid: u32) {
    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;

    let nix_pid = Pid::from_raw(i32::try_from(pid).unwrap_or(i32::MAX));

    if kill(nix_pid, Signal::SIGTERM).is_err() {
        return;
    }
    tracing::info!(pid, "sent SIGTERM");
    std::thread::sleep(std::time::Duration::from_secs(2));

    if kill(nix_pid, None).is_ok() {
        let _ = kill(nix_pid, Signal::SIGKILL);
        tracing::info!(pid, "sent SIGKILL");
    }
}

// ---------------------------------------------------------------------------
// Image preparation helpers
// ---------------------------------------------------------------------------

/// Prepares a container rootfs at `{data_dir}/rootfs/{container_id}` from
/// the given image source URI.
///
/// Supported sources:
/// - `file://<path>` — bind-mounts or copies the directory as rootfs
/// - `tar://<path>` — extracts the archive into the rootfs directory
///
/// # Errors
///
/// Returns an error if the image source is unsupported or extraction fails.
fn prepare_rootfs(image_uri: &str, container_id: &ContainerId) -> Result<PathBuf> {
    let data_dir = containust_common::constants::data_dir();
    let rootfs_dir = data_dir.join("rootfs").join(container_id.as_str());

    // If rootfs already exists from a previous create, reuse it
    if rootfs_dir.exists() {
        tracing::info!(path = %rootfs_dir.display(), "reusing existing rootfs");
        return Ok(rootfs_dir);
    }

    if let Some(path_str) = image_uri.strip_prefix("file://") {
        let src = PathBuf::from(path_str);
        if !src.exists() {
            return Err(ContainustError::NotFound {
                kind: "image directory",
                id: path_str.to_string(),
            });
        }
        copy_dir_recursive(&src, &rootfs_dir)?;
        tracing::info!(rootfs = %rootfs_dir.display(), "rootfs copied from file:// source");
    } else if let Some(path_str) = image_uri.strip_prefix("tar://") {
        let archive = PathBuf::from(path_str);
        if !archive.exists() {
            return Err(ContainustError::NotFound {
                kind: "tar archive",
                id: path_str.to_string(),
            });
        }
        extract_tar(&archive, &rootfs_dir)?;
        tracing::info!(rootfs = %rootfs_dir.display(), "rootfs extracted from tar:// source");
    } else {
        return Err(ContainustError::Config {
            message: format!("unsupported image source for Linux native: {image_uri}"),
        });
    }

    Ok(rootfs_dir)
}

/// Copies a directory tree recursively without following symlinks.
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst).map_err(|e| ContainustError::Io {
        path: dst.to_path_buf(),
        source: e,
    })?;

    for entry in std::fs::read_dir(src).map_err(|e| ContainustError::Io {
        path: src.to_path_buf(),
        source: e,
    })? {
        let entry = entry.map_err(|e| ContainustError::Io {
            path: src.to_path_buf(),
            source: e,
        })?;
        let file_type = entry.file_type().map_err(|e| ContainustError::Io {
            path: src.to_path_buf(),
            source: e,
        })?;
        let dest_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            let _ = std::fs::copy(entry.path(), &dest_path).map_err(|e| ContainustError::Io {
                path: entry.path(),
                source: e,
            })?;
        }
    }

    Ok(())
}

/// Extracts a tar archive into a target directory.
fn extract_tar(archive: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst).map_err(|e| ContainustError::Io {
        path: dst.to_path_buf(),
        source: e,
    })?;

    let tar_gz = std::fs::File::open(archive).map_err(|e| ContainustError::Io {
        path: archive.to_path_buf(),
        source: e,
    })?;

    // Try gzip first, fall back to plain tar
    let size = tar_gz.metadata().map_or(0, |m| m.len());
    if size > 0 {
        // Peek first bytes to detect gzip magic (1f 8b)
        use std::io::Read;
        let mut reader = std::io::BufReader::new(tar_gz);
        let mut header = [0u8; 2];
        if reader.read_exact(&mut header).is_ok() && header == [0x1f, 0x8b] {
            // Gzipped tar
            let decoder = flate2::read::GzDecoder::new(reader);
            let mut archive = tar::Archive::new(decoder);
            archive.unpack(dst).map_err(|e| ContainustError::Config {
                message: format!("tar.gz extraction failed: {e}"),
            })?;
        } else {
            // Plain tar
            use std::io::Seek;
            let _ = reader.seek(std::io::SeekFrom::Start(0));
            let mut archive = tar::Archive::new(reader);
            archive.unpack(dst).map_err(|e| ContainustError::Config {
                message: format!("tar extraction failed: {e}"),
            })?;
        }
    }

    Ok(())
}

/// Derives a default shell command from the image source.
///
/// Defaults to `["sh"]` for images without a CMD manifest.
fn derive_command_from_image(_image_uri: &str) -> Vec<String> {
    vec!["sh".to_string()]
}

// ---------------------------------------------------------------------------
// Cgroup management helpers
// ---------------------------------------------------------------------------

/// Attempt to apply cgroup resource limits for a container.
/// Non-fatal on failure — containers still run without limits.
#[allow(clippy::missing_const_for_fn)]
fn apply_cgroup_limits(
    container_id: &ContainerId,
    pid: u32,
    memory_bytes: Option<u64>,
    cpu_shares: Option<u64>,
) {
    #[cfg(target_os = "linux")]
    {
        use containust_common::types::ResourceLimits;
        use containust_core::cgroup::CgroupManager;

        match CgroupManager::create(container_id.as_str()) {
            Ok(mgr) => {
                let limits = ResourceLimits {
                    memory_bytes,
                    cpu_shares,
                    io_weight: None,
                };
                let _ = mgr.apply_limits(&limits).map_err(|e| {
                    tracing::warn!(%e, "failed to apply cgroup limits");
                });
                let _ = mgr.add_process(pid).map_err(|e| {
                    tracing::warn!(%e, "failed to add process to cgroup");
                });
            }
            Err(e) => {
                tracing::warn!(%e, "cgroup creation failed (containers run without resource limits)");
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (container_id, pid, memory_bytes, cpu_shares);
    }
}

/// Best-effort cgroup cleanup during container stop.
fn cleanup_cgroup(container_id: &ContainerId) {
    let path = PathBuf::from(containust_common::constants::CGROUP_V2_PATH)
        .join("containust")
        .join(container_id.as_str());
    if path.exists() {
        let _ = std::fs::remove_dir_all(&path);
        tracing::debug!(path = %path.display(), "cgroup cleaned up");
    }
}

#[cfg(test)]
mod tests {
    #![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

    use super::*;

    #[test]
    fn derive_command_returns_default_sh() {
        let cmd = derive_command_from_image("file:///some/image");
        assert_eq!(cmd, vec!["sh"]);
    }

    #[test]
    fn derive_command_empty_uri_returns_sh() {
        let cmd = derive_command_from_image("");
        assert_eq!(cmd, vec!["sh"]);
    }

    #[test]
    fn derive_command_tar_uri_returns_sh() {
        let cmd = derive_command_from_image("tar:///archive.tar.gz");
        assert_eq!(cmd, vec!["sh"]);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_native_backend_new_creates_instance() {
        let backend = LinuxNativeBackend::new();
        assert!(backend.is_available());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_native_backend_default_creates_instance() {
        let backend = LinuxNativeBackend::default();
        assert!(backend.is_available());
    }

    #[test]
    fn copy_dir_recursive_copies_files() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("containust_test_copy");
        let src = temp_dir.join("src");
        let dst = temp_dir.join("dst");

        // Setup
        std::fs::create_dir_all(&src).expect("create src dir");
        std::fs::write(src.join("file.txt"), "hello").expect("write file");
        std::fs::create_dir_all(src.join("sub")).expect("create subdir");
        std::fs::write(src.join("sub").join("nested.txt"), "nested").expect("write nested file");

        copy_dir_recursive(&src, &dst)?;

        assert!(dst.join("file.txt").exists());
        assert!(dst.join("sub").join("nested.txt").exists());
        assert_eq!(
            std::fs::read_to_string(dst.join("file.txt")).expect("read"),
            "hello"
        );
        assert_eq!(
            std::fs::read_to_string(dst.join("sub").join("nested.txt")).expect("read"),
            "nested"
        );

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn copy_dir_recursive_empty_directory() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("containust_test_empty_copy");
        let src = temp_dir.join("empty_src");
        let dst = temp_dir.join("empty_dst");

        std::fs::create_dir_all(&src).expect("create src");
        copy_dir_recursive(&src, &dst)?;

        assert!(dst.exists());
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn cleanup_cgroup_does_not_panic_on_missing_dir() {
        let id = ContainerId::new("nonexistent-cgroup");
        // Should not panic even if cgroup path does not exist
        cleanup_cgroup(&id);
    }

    #[test]
    fn terminate_process_does_not_panic_on_invalid_pid() {
        // Use a PID that almost certainly does not exist
        terminate_process(999_999_999);
    }
}
