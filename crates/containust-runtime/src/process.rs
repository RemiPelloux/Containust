//! Process spawning inside isolated namespaces with full container
//! root filesystem preparation (bind-mount, `pivot_root`, capability dropping).

#![allow(clippy::print_stdout, clippy::print_stderr, unsafe_code, missing_docs)]

use containust_common::error::{ContainustError, Result};
use std::path::Path;

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
/// On Linux, this forks, unshares mount namespace, bind-mounts
/// the rootfs, pivots root, mounts pseudo-filesystems, drops
/// capabilities, then execs.
///
/// # Errors
///
/// Returns an error if namespace creation, mount, pivot_root, or exec fails.
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

    let container_root = rootfs.to_path_buf();
    if !container_root.exists() {
        return Err(ContainustError::Config {
            message: format!(
                "rootfs directory does not exist: {}",
                container_root.display()
            ),
        });
    }

    tracing::info!(
        command = ?command,
        rootfs = %container_root.display(),
        "spawning container process"
    );

    let mut child_cmd = std::process::Command::new(&command[0]);
    if command.len() > 1 {
        let _ = child_cmd.args(&command[1..]);
    }

    // Clear host environment and set minimal defaults
    let _ = child_cmd.env_clear();
    let _ = child_cmd.env("PATH", "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin");
    let _ = child_cmd.env("HOME", "/root");
    let _ = child_cmd.env("TERM", "xterm");
    for (key, value) in env {
        let _ = child_cmd.env(key, value);
    }

    let rootfs_owned = container_root.clone();

    // SAFETY: pre_exec runs in the child between fork and exec.
    // Operations performed:
    // 1. unshare(CLONE_NEWNS) — isolate mount namespace
    // 2. bind-mount rootfs — make it a mount point
    // 3. pivot_root — switch root filesystem
    // 4. mount proc, sys, dev — pseudo-filesystems
    // 5. drop all capabilities — least-privilege execution
    unsafe {
        let _ = child_cmd.pre_exec(move || {
            // 1. Isolate mount namespace
            nix::sched::unshare(nix::sched::CloneFlags::CLONE_NEWNS).map_err(|e| {
                std::io::Error::other(format!("unshare mount namespace failed (root?): {e}"))
            })?;

            // 2. Mount private/slave so changes don't propagate to host
            use nix::mount::{MsFlags, mount};
            let _ = mount::<str, str, str, str>(
                None,
                "/",
                None,
                MsFlags::MS_REC | MsFlags::MS_SLAVE,
                None,
            );

            // 3. Bind-mount rootfs onto itself (required for pivot_root)
            mount(
                Some(&rootfs_owned),
                &rootfs_owned,
                None::<&str>,
                MsFlags::MS_BIND | MsFlags::MS_REC,
                None::<&str>,
            )
            .map_err(|e| std::io::Error::other(format!("bind mount rootfs failed: {e}")))?;

            // 4. Pivot root
            containust_core::filesystem::pivot_root::pivot_root(
                Path::new("/"),
                Path::new("/.old_root"),
            )
            .map_err(|e| std::io::Error::other(format!("pivot_root failed: {e}")))?;

            // 5. Mount pseudo-filesystems
            mount_pseudo_filesystems()?;

            // 6. Drop all capabilities
            let _ = containust_core::capability::drop_capabilities(&[]);

            Ok(())
        });
    }

    let child = child_cmd.spawn().map_err(|e| ContainustError::Io {
        path: container_root.clone(),
        source: e,
    })?;

    let pid = child.id();
    tracing::info!(pid, "container process spawned");
    Ok(pid)
}

/// Mounts essential pseudo-filesystems after pivot_root.
#[cfg(target_os = "linux")]
fn mount_pseudo_filesystems() -> std::io::Result<()> {
    use nix::mount::{MsFlags, mount};

    let pseudo_mounts: &[(&str, &str, &str, MsFlags, Option<&str>)] = &[
        (
            "/proc",
            "proc",
            "proc",
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
            None,
        ),
        (
            "/sys",
            "sysfs",
            "sysfs",
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC | MsFlags::MS_RDONLY,
            None,
        ),
        (
            "/dev",
            "tmpfs",
            "tmpfs",
            MsFlags::MS_NOSUID | MsFlags::MS_STRICTATIME,
            Some("mode=755,size=65536k"),
        ),
        (
            "/dev/pts",
            "devpts",
            "devpts",
            MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC,
            Some("newinstance,ptmxmode=0666,mode=0620,gid=5"),
        ),
        (
            "/dev/shm",
            "tmpfs",
            "tmpfs",
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
            Some("mode=1777,size=65536k"),
        ),
        (
            "/tmp",
            "tmpfs",
            "tmpfs",
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
            Some("mode=1777,size=65536k"),
        ),
    ];

    for (path, src, fstype, flags, opts) in pseudo_mounts {
        std::fs::create_dir_all(path).ok();
        mount(Some(*src), *path, Some(*fstype), *flags, *opts)
            .map_err(|e| std::io::Error::other(format!("mount {} failed: {e}", path)))?;
    }

    // Create essential device nodes as empty files (real devices
    // require mknod which needs CAP_MKNOD in user namespace)
    for dev in &[
        "/dev/null",
        "/dev/zero",
        "/dev/random",
        "/dev/urandom",
        "/dev/tty",
    ] {
        if !std::path::Path::new(dev).exists() {
            let _ = std::fs::write(dev, &[]);
        }
    }

    Ok(())
}

/// Non-Linux stub.
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
