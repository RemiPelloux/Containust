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
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
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
/// Returns an error if namespace creation, mount, pivot_root, or exec fails.
#[cfg(target_os = "linux")]
pub fn spawn_container_process(config: &ProcessConfig) -> Result<u32> {
    use std::os::unix::process::CommandExt;

    if config.command.is_empty() {
        return Err(ContainustError::Config {
            message: "container command is empty".into(),
        });
    }

    let container_root = config.rootfs.clone();
    if !container_root.exists() {
        return Err(ContainustError::Config {
            message: format!(
                "rootfs directory does not exist: {}",
                container_root.display()
            ),
        });
    }

    tracing::info!(
        command = ?config.command,
        rootfs = %container_root.display(),
        "spawning container process"
    );

    let mut child_cmd = std::process::Command::new(&config.command[0]);
    if config.command.len() > 1 {
        let _ = child_cmd.args(&config.command[1..]);
    }

    // Clear host environment and set minimal defaults
    let _ = child_cmd.env_clear();
    let _ = child_cmd.env("PATH", "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin");
    let _ = child_cmd.env("HOME", "/root");
    let _ = child_cmd.env("TERM", "xterm");
    for (key, value) in &config.env {
        let _ = child_cmd.env(key, value);
    }

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

            for volume in &volumes {
                bind_volume(volume, &rootfs_owned)?;
            }

            // 4. Pivot root
            containust_core::filesystem::pivot_root::pivot_root(
                &rootfs_owned,
                &rootfs_owned.join(".old_root"),
            )
            .map_err(|e| std::io::Error::other(format!("pivot_root failed: {e}")))?;

            // 5. Mount pseudo-filesystems
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

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
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
        let _ = std::fs::create_dir_all(path);
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
pub fn spawn_container_process(_config: &ProcessConfig) -> Result<u32> {
    Err(ContainustError::Config {
        message: "process spawning requires Linux (use VM backend on macOS/Windows)".into(),
    })
}

#[cfg(test)]
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
