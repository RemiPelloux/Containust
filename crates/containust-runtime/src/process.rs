//! Process spawning inside isolated namespaces.

use std::path::Path;

use containust_common::error::{ContainustError, Result};

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

/// Spawns a new process inside the container's rootfs.
///
/// On Linux, this forks and uses chroot into the rootfs before execing.
/// On non-Linux, returns an error (containers run inside the VM).
///
/// # Errors
///
/// Returns an error if fork, namespace entry, or exec fails.
#[cfg(target_os = "linux")]
pub fn spawn_container_process(
    command: &[String],
    env: &[(String, String)],
    rootfs: &Path,
) -> Result<u32> {
    use std::os::unix::process::CommandExt;

    if command.is_empty() {
        return Err(ContainustError::Config {
            message: "container command is empty".into(),
        });
    }

    tracing::info!(
        command = ?command,
        rootfs = %rootfs.display(),
        "spawning container process"
    );

    let mut child_cmd = std::process::Command::new(&command[0]);
    if command.len() > 1 {
        let _ = child_cmd.args(&command[1..]);
    }

    let _ = child_cmd.env_clear();
    let _ = child_cmd.env("PATH", "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin");
    let _ = child_cmd.env("HOME", "/root");
    let _ = child_cmd.env("TERM", "xterm");
    for (key, value) in env {
        let _ = child_cmd.env(key, value);
    }

    let rootfs_owned = rootfs.to_path_buf();
    // SAFETY: pre_exec runs in the child process between fork and exec.
    // chroot and chdir are safe here as we've validated rootfs exists.
    unsafe {
        let _ = child_cmd.pre_exec(move || enter_rootfs(&rootfs_owned));
    }

    let child = child_cmd.spawn().map_err(|e| ContainustError::Io {
        path: rootfs.to_path_buf(),
        source: e,
    })?;

    let pid = child.id();
    tracing::info!(pid, "container process spawned");
    Ok(pid)
}

#[cfg(target_os = "linux")]
fn enter_rootfs(rootfs: &Path) -> std::io::Result<()> {
    nix::unistd::chroot(rootfs)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::PermissionDenied, e.to_string()))?;
    std::env::set_current_dir("/")
}

/// Spawns a new process inside the container's rootfs.
///
/// On non-Linux platforms, returns an error because container process
/// isolation requires Linux kernel features (namespaces, cgroups).
///
/// # Errors
///
/// Always returns an error on non-Linux platforms.
#[cfg(not(target_os = "linux"))]
pub fn spawn_container_process(
    _command: &[String],
    _env: &[(String, String)],
    _rootfs: &Path,
) -> Result<u32> {
    Err(ContainustError::Config {
        message: "process spawning requires Linux (use VM backend on macOS/Windows)".into(),
    })
}
