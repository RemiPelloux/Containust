//! Process spawning inside isolated namespaces with full container
//! root filesystem preparation (bind-mount, `pivot_root`, capability dropping).

#![allow(clippy::print_stdout, clippy::print_stderr, unsafe_code, missing_docs)]

use containust_common::error::{ContainustError, Result};
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
/// On Linux, this forks, unshares mount namespace, bind-mounts
/// the rootfs, pivots root, mounts pseudo-filesystems, drops
/// capabilities, then execs.
///
/// # Errors
///
/// Returns an error if namespace creation, mount, `pivot_root`, or exec fails.
#[cfg(target_os = "linux")]
pub fn spawn_container_process(config: &ProcessConfig) -> Result<u32> {
    use std::os::unix::process::CommandExt;

    let container_root = config.rootfs.clone();
    tracing::info!(
        command = ?config.command,
        rootfs = %container_root.display(),
        "spawning container process"
    );
    let mut child_cmd = prepare_child_command(config)?;

    let rootfs_owned = container_root.clone();
    let volumes = config.volumes.clone();
    let readonly_rootfs = config.readonly_rootfs;

    // SAFETY: pre_exec runs in the child between fork and exec.
    // Operations performed:
    // 1. unshare(CLONE_NEWNS) — isolate mount namespace
    // 2. bind-mount rootfs — make it a mount point
    // 3. pivot_root — switch root filesystem
    // 4. mount proc, sys, dev — pseudo-filesystems
    // 5. drop all capabilities — least-privilege execution
    unsafe {
        let _ = child_cmd
            .pre_exec(move || configure_child_isolation(&rootfs_owned, &volumes, readonly_rootfs));
    }

    let child = child_cmd.spawn().map_err(|e| ContainustError::Io {
        path: container_root.clone(),
        source: e,
    })?;

    let pid = child.id();
    tracing::info!(pid, "container process spawned");
    Ok(pid)
}

#[cfg(target_os = "linux")]
fn prepare_child_command(config: &ProcessConfig) -> Result<std::process::Command> {
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
    Ok(command)
}

#[cfg(target_os = "linux")]
fn configure_child_isolation(
    rootfs: &Path,
    volumes: &[String],
    readonly_rootfs: bool,
) -> std::io::Result<()> {
    use nix::mount::{MsFlags, mount};

    nix::sched::unshare(nix::sched::CloneFlags::CLONE_NEWNS).map_err(|e| {
        std::io::Error::other(format!("unshare mount namespace failed (root?): {e}"))
    })?;
    let _ = mount::<str, str, str, str>(None, "/", None, MsFlags::MS_REC | MsFlags::MS_SLAVE, None);
    for volume in volumes {
        bind_volume(volume, rootfs)?;
    }
    containust_core::filesystem::pivot_root::pivot_root(rootfs, &rootfs.join(".old_root"))
        .map_err(|e| std::io::Error::other(format!("pivot_root failed: {e}")))?;
    mount_pseudo_filesystems()?;
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
    let _ = containust_core::capability::drop_capabilities(&[]);
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
    let mut parts = spec.split(':');
    let source = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();
    let mode = parts.next();
    if source.is_empty() || target.is_empty() || parts.next().is_some() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid volume specification: {spec}"),
        ));
    }
    let source_path = std::path::Path::new(source);
    let target_path = std::path::Path::new(target);
    if !source_path.is_absolute()
        || !target_path.is_absolute()
        || target_path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        || mode.is_some_and(|value| value != "ro" && value != "rw")
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("unsafe volume specification: {spec}"),
        ));
    }
    Ok(VolumeMount {
        source: source_path.to_path_buf(),
        target: target_path.to_path_buf(),
        readonly: mode == Some("ro"),
    })
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

#[cfg(target_os = "linux")]
type PseudoMount = (
    &'static str,
    &'static str,
    &'static str,
    nix::mount::MsFlags,
    Option<&'static str>,
);

#[cfg(target_os = "linux")]
fn pseudo_mounts() -> [PseudoMount; 6] {
    use nix::mount::MsFlags;

    [
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
    ]
}

/// Mounts essential pseudo-filesystems after `pivot_root`.
#[cfg(target_os = "linux")]
fn mount_pseudo_filesystems() -> std::io::Result<()> {
    use nix::mount::mount;

    for (path, src, fstype, flags, opts) in pseudo_mounts() {
        let _ = std::fs::create_dir_all(path);
        mount(Some(src), path, Some(fstype), flags, opts)
            .map_err(|e| std::io::Error::other(format!("mount {path} failed: {e}")))?;
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
            let _ = std::fs::write(dev, []);
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
pub fn spawn_container_process(_config: &ProcessConfig) -> Result<u32> {
    Err(ContainustError::Config {
        message: "process spawning requires Linux (use VM backend on macOS/Windows)".into(),
    })
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn volume_spec_parses_readonly_mount() {
        let volume = parse_volume_spec("/host/data:/app/data:ro").expect("parse");
        assert_eq!(volume.source, std::path::Path::new("/host/data"));
        assert_eq!(volume.target, std::path::Path::new("/app/data"));
        assert!(volume.readonly);
    }

    #[test]
    fn volume_spec_rejects_relative_and_parent_paths() {
        assert!(parse_volume_spec("relative:/data").is_err());
        assert!(parse_volume_spec("/host:/app/../escape").is_err());
        assert!(parse_volume_spec("/host:/data:invalid").is_err());
    }

    #[test]
    fn volume_spec_rejects_missing_fields() {
        assert!(parse_volume_spec("/host-only").is_err());
        assert!(parse_volume_spec(":/data").is_err());
        assert!(parse_volume_spec("/host:").is_err());
    }
}
