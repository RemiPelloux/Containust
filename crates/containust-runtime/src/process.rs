//! Process spawning inside isolated namespaces with full container
//! root filesystem preparation (bind-mount, `pivot_root`, capability dropping).

#![allow(clippy::print_stdout, clippy::print_stderr, unsafe_code, missing_docs)]

use containust_common::error::{ContainustError, Result};
use containust_core::namespace::NamespaceConfig;
#[cfg(target_os = "linux")]
use std::path::Path;

/// Complete process setup requested for a container init process.
#[derive(Debug, Clone)]
pub struct ProcessConfig {
    /// Command and arguments.
    pub command: Vec<String>,
    /// Environment variables.
    pub env: Vec<(String, String)>,
    /// Root filesystem path.
    pub rootfs: std::path::PathBuf,
    /// Whether the root mount should be read-only.
    pub readonly_rootfs: bool,
    /// Host-to-container bind mounts.
    pub volumes: Vec<String>,
    /// Namespace isolation policy.
    pub namespaces: NamespaceConfig,
    /// When set, join this netns instead of `unshare(CLONE_NEWNET)`.
    pub join_netns: Option<std::path::PathBuf>,
    /// Log file receiving the container's stdout/stderr.
    ///
    /// `None` inherits the parent's stdio (foreground debugging only);
    /// backends set this so detached containers do not hold the CLI's
    /// output pipes open.
    pub log_path: Option<std::path::PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
#[cfg(target_os = "linux")]
struct VolumeMount {
    source: std::path::PathBuf,
    target: std::path::PathBuf,
    readonly: bool,
}

/// Output captured from a spawned process.
#[derive(Debug, Clone)]
pub struct ProcessOutput {
    /// Process ID.
    pub pid: u32,
    /// Standard output (if captured).
    pub stdout: String,
    /// Standard error (if captured).
    pub stderr: String,
    /// Exit code (if process has exited).
    pub exit_code: Option<i32>,
}

/// Spawns a new process inside the container's rootfs with full
/// namespace isolation and capability dropping.
///
/// # Errors
///
/// Returns an error if namespace creation, mount, `pivot_root`, capability
/// drop, or exec fails. Capability and namespace failures fail closed.
#[cfg(target_os = "linux")]
pub fn spawn_container_process(config: &ProcessConfig) -> Result<u32> {
    use std::os::unix::process::CommandExt;

    config.namespaces.validate_for_spawn()?;
    let _ = crate::volume::validate_volumes(&config.volumes)?;

    tracing::info!(
        command = ?config.command,
        rootfs = %config.rootfs.display(),
        user = config.namespaces.user,
        pid = config.namespaces.pid,
        "spawning container process"
    );

    if config.namespaces.user || config.namespaces.pid {
        return crate::process_spawn::spawn_with_user_pid(config);
    }

    let container_root = config.rootfs.clone();
    let mut child_cmd = prepare_child_command_for_spawn(config)?;
    let rootfs_owned = container_root.clone();
    let volumes = config.volumes.clone();
    let readonly_rootfs = config.readonly_rootfs;
    let namespaces = config.namespaces.clone();

    // SAFETY: pre_exec runs in the child between fork and exec.
    unsafe {
        let _ = child_cmd.pre_exec(move || {
            configure_child_isolation(&rootfs_owned, &volumes, readonly_rootfs, &namespaces)
        });
    }

    let child = child_cmd.spawn().map_err(|e| ContainustError::Io {
        path: container_root.clone(),
        source: e,
    })?;

    let pid = child.id();
    tracing::info!(pid, "container process spawned");
    std::mem::forget(child);
    Ok(pid)
}

/// Builds the child `Command` (env, stdio, argv). Shared with the user/PID spawn path.
#[cfg(target_os = "linux")]
pub(crate) fn prepare_child_command_for_spawn(
    config: &ProcessConfig,
) -> Result<std::process::Command> {
    if config.command.is_empty() {
        return Err(ContainustError::Config {
            message: "container command is empty".into(),
        });
    }
    if !config.rootfs.exists() {
        return Err(ContainustError::Config {
            message: format!(
                "rootfs directory does not exist: {}",
                config.rootfs.display()
            ),
        });
    }

    let mut command = std::process::Command::new(&config.command[0]);
    let _ = command.args(&config.command[1..]);
    let _ = command.env_clear();
    let _ = command.env("PATH", "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin");
    let _ = command.env("HOME", "/root");
    let _ = command.env("TERM", "xterm");
    for (key, value) in &config.env {
        let _ = command.env(key, value);
    }
    let _ = command.stdin(std::process::Stdio::null());
    if let Some(log_path) = &config.log_path {
        let log_file = open_log_sink(log_path)?;
        let stderr_file = log_file.try_clone().map_err(|source| ContainustError::Io {
            path: log_path.clone(),
            source,
        })?;
        let _ = command.stdout(log_file);
        let _ = command.stderr(stderr_file);
    }
    Ok(command)
}

/// Opens the container log file for appending, creating parents as needed.
#[cfg(target_os = "linux")]
fn open_log_sink(log_path: &Path) -> Result<std::fs::File> {
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ContainustError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|source| ContainustError::Io {
            path: log_path.to_path_buf(),
            source,
        })
}

#[cfg(target_os = "linux")]
fn configure_child_isolation(
    rootfs: &Path,
    volumes: &[String],
    readonly_rootfs: bool,
    namespaces: &NamespaceConfig,
) -> std::io::Result<()> {
    containust_core::namespace::create_namespaces(namespaces)
        .map_err(|e| std::io::Error::other(format!("namespace creation failed: {e}")))?;
    configure_child_isolation_after_ns(rootfs, volumes, readonly_rootfs)
}

/// Mount / `pivot_root` / capability drop after namespaces already exist.
#[cfg(target_os = "linux")]
pub(crate) fn configure_child_isolation_after_ns(
    rootfs: &Path,
    volumes: &[String],
    readonly_rootfs: bool,
) -> std::io::Result<()> {
    use nix::mount::{MsFlags, mount};

    let _ = mount::<str, str, str, str>(None, "/", None, MsFlags::MS_REC | MsFlags::MS_SLAVE, None);
    for volume in volumes {
        bind_volume(volume, rootfs)?;
    }
    // Mount proc/sys/dev under rootfs *before* pivot so a host proc-anchor
    // remains visible (userns `mount_too_revealing` check).
    crate::process_mounts::mount_pseudo_filesystems_at(rootfs)?;
    containust_core::filesystem::pivot_root::pivot_root(rootfs, &rootfs.join(".old_root"))
        .map_err(|e| std::io::Error::other(format!("pivot_root failed: {e}")))?;
    if readonly_rootfs {
        mount(
            None::<&str>,
            "/",
            None::<&str>,
            MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY,
            None::<&str>,
        )
        .map_err(|e| std::io::Error::other(format!("read-only rootfs failed: {e}")))?;
    }
    containust_core::capability::drop_capabilities(&[])
        .map_err(|e| std::io::Error::other(format!("capability drop failed: {e}")))?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn bind_volume(spec: &str, rootfs: &Path) -> std::io::Result<()> {
    use nix::mount::{MsFlags, mount};

    let volume = parse_volume_spec(spec)?;
    if !volume.source.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("volume source does not exist: {}", volume.source.display()),
        ));
    }
    let relative_target = volume.target.strip_prefix("/").map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid volume target")
    })?;
    let target = rootfs.join(relative_target);
    prepare_volume_target(&volume.source, &target)?;
    mount(
        Some(&volume.source),
        &target,
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REC,
        None::<&str>,
    )
    .map_err(|e| std::io::Error::other(format!("bind volume failed: {e}")))?;
    if volume.readonly {
        mount(
            None::<&str>,
            &target,
            None::<&str>,
            MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY,
            None::<&str>,
        )
        .map_err(|e| std::io::Error::other(format!("read-only volume failed: {e}")))?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn parse_volume_spec(spec: &str) -> std::io::Result<VolumeMount> {
    crate::volume::parse_and_validate_volume(spec)
        .map(|mount| VolumeMount {
            source: mount.source,
            target: mount.target,
            readonly: mount.readonly,
        })
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string()))
}

#[cfg(target_os = "linux")]
fn prepare_volume_target(source: &Path, target: &Path) -> std::io::Result<()> {
    if source.is_dir() {
        return std::fs::create_dir_all(target);
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(target)?;
    Ok(())
}

/// Non-Linux stub.
///
/// # Errors
///
/// Always returns an error on non-Linux platforms.
#[cfg(not(target_os = "linux"))]
pub fn spawn_container_process(_config: &ProcessConfig) -> Result<u32> {
    Err(ContainustError::Config {
        message: "process spawning requires Linux (use VM backend on macOS/Windows)".into(),
    })
}

/// Grace period between SIGTERM and SIGKILL when stopping a container.
pub(crate) const STOP_GRACE: std::time::Duration = std::time::Duration::from_secs(2);

/// Interval at which a terminating process is polled for exit.
#[cfg(target_os = "linux")]
const STOP_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(20);

/// Sends SIGTERM, waits up to `grace` for the process to exit (polling),
/// then sends SIGKILL if it is still alive. `force` sends SIGKILL at once.
///
/// Returns as soon as the process has exited instead of always sleeping
/// for the full grace period.
#[cfg(target_os = "linux")]
pub(crate) fn terminate_process(pid: u32, force: bool, grace: std::time::Duration) {
    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;

    let nix_pid = Pid::from_raw(i32::try_from(pid).unwrap_or(i32::MAX));

    if force {
        let _ = kill(nix_pid, Signal::SIGKILL);
        return;
    }
    if kill(nix_pid, Signal::SIGTERM).is_err() {
        return;
    }
    tracing::info!(pid, "sent SIGTERM");

    let deadline = std::time::Instant::now() + grace;
    loop {
        if process_has_exited(nix_pid) {
            return;
        }
        let now = std::time::Instant::now();
        if now >= deadline {
            break;
        }
        std::thread::sleep(STOP_POLL_INTERVAL.min(deadline - now));
    }

    if kill(nix_pid, None).is_ok() {
        let _ = kill(nix_pid, Signal::SIGKILL);
        tracing::info!(pid, "sent SIGKILL");
    }
}

/// Non-Linux stub: process signaling is handled by the VM backend.
#[cfg(not(target_os = "linux"))]
pub(crate) const fn terminate_process(_pid: u32, _force: bool, _grace: std::time::Duration) {}

/// Reports whether `pid` has exited.
///
/// For our own children a zombie still answers `kill(pid, 0)`, so check with
/// `waitid(WNOWAIT)` first (which does not reap, leaving that to the owner).
/// Non-children fall back to `kill(pid, 0)`; `EPERM` means it is alive.
#[cfg(target_os = "linux")]
fn process_has_exited(pid: nix::unistd::Pid) -> bool {
    use nix::errno::Errno;
    use nix::sys::wait::{Id, WaitPidFlag, WaitStatus, waitid};

    match waitid(
        Id::Pid(pid),
        WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT,
    ) {
        Ok(WaitStatus::StillAlive) => false,
        Ok(_) => true,
        Err(_) => matches!(nix::sys::signal::kill(pid, None), Err(e) if e != Errno::EPERM),
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn terminate_returns_promptly_when_process_exits_on_sigterm() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("spawn sleep");
        let started = std::time::Instant::now();
        terminate_process(child.id(), false, STOP_GRACE);
        let elapsed = started.elapsed();
        let status = child.wait().expect("reap sleep");

        assert!(!status.success(), "sleep should be killed by a signal");
        assert!(
            elapsed < std::time::Duration::from_millis(1_000),
            "stop took {elapsed:?}; expected well under the {STOP_GRACE:?} grace"
        );
    }

    #[test]
    fn terminate_escalates_to_sigkill_after_grace() {
        // `sh` ignoring SIGTERM forces the SIGKILL path.
        let mut child = std::process::Command::new("sh")
            .args(["-c", "trap '' TERM; while :; do sleep 0.05; done"])
            .spawn()
            .expect("spawn sh");
        std::thread::sleep(std::time::Duration::from_millis(100));
        let grace = std::time::Duration::from_millis(200);
        let started = std::time::Instant::now();
        terminate_process(child.id(), false, grace);
        let elapsed = started.elapsed();
        let status = child.wait().expect("reap sh");

        assert!(!status.success());
        assert!(elapsed >= grace, "returned before grace: {elapsed:?}");
    }

    #[test]
    fn terminate_ignores_missing_pid() {
        terminate_process(999_999_999, false, STOP_GRACE);
    }
}
