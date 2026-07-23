//! Pseudo-filesystem mounts for container root filesystems.

#![allow(missing_docs)]

#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};

/// Fully-visible procfs mount used as a `mount_too_revealing` anchor.
///
/// Kernels refuse `mount -t proc` inside a non-init user namespace when every
/// existing proc mount in the mount namespace is masked (common on CI runners
/// and nested containers). A fresh proc mount created in the **init** user
/// namespace satisfies the check so the container can mount its own proc.
#[cfg(target_os = "linux")]
pub(crate) const PROC_ANCHOR_PATH: &str = "/run/containust/proc-anchor";

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
            "proc",
            "proc",
            "proc",
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
            None,
        ),
        (
            "sys",
            "sysfs",
            "sysfs",
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC | MsFlags::MS_RDONLY,
            None,
        ),
        (
            "dev",
            "tmpfs",
            "tmpfs",
            MsFlags::MS_NOSUID | MsFlags::MS_STRICTATIME,
            Some("mode=755,size=65536k"),
        ),
        // Avoid `gid=5` — unmapped in single-UID user namespaces (range 1).
        (
            "dev/pts",
            "devpts",
            "devpts",
            MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC,
            Some("newinstance,ptmxmode=0666,mode=0620"),
        ),
        (
            "dev/shm",
            "tmpfs",
            "tmpfs",
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
            Some("mode=1777,size=65536k"),
        ),
        (
            "tmp",
            "tmpfs",
            "tmpfs",
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
            Some("mode=1777,size=65536k"),
        ),
    ]
}

/// Ensures a fully-visible procfs exists for subsequent userns proc mounts.
///
/// Safe to call repeatedly; idempotent when the anchor is already mounted.
///
/// # Errors
///
/// Returns an error when the directory cannot be created or the mount fails
/// for a reason other than the anchor already being mounted.
#[cfg(target_os = "linux")]
pub(crate) fn ensure_visible_proc_anchor() -> std::io::Result<()> {
    use nix::errno::Errno;
    use nix::mount::{MsFlags, mount};

    let path = Path::new(PROC_ANCHOR_PATH);
    std::fs::create_dir_all(path)?;
    if path_is_proc_mount(path) {
        return Ok(());
    }
    let flags = MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC;
    match mount(Some("proc"), path, Some("proc"), flags, None::<&str>) {
        Ok(()) => Ok(()),
        Err(Errno::EBUSY) if path_is_proc_mount(path) => Ok(()),
        Err(err) => Err(std::io::Error::other(format!(
            "failed to mount visible proc anchor at {PROC_ANCHOR_PATH}: {err}"
        ))),
    }
}

#[cfg(target_os = "linux")]
fn path_is_proc_mount(path: &Path) -> bool {
    let Ok(info) = std::fs::read_to_string("/proc/self/mountinfo") else {
        return false;
    };
    let target = path.to_string_lossy();
    info.lines().any(|line| {
        let mut fields = line.split(' ');
        // mountinfo: id parent major:minor root mountpoint ... - fstype ...
        let mount_point = fields.nth(4);
        let fstype = fields.find(|f| *f == "-").and_then(|_| fields.next());
        matches!((mount_point, fstype), (Some(mp), Some("proc")) if mp == target.as_ref())
    })
}

/// Mounts essential pseudo-filesystems under `rootfs` (before `pivot_root`).
///
/// Mounting before pivot keeps the init-userns proc anchor visible so the
/// kernel's `mount_too_revealing` check can succeed under a user namespace.
#[cfg(target_os = "linux")]
pub fn mount_pseudo_filesystems_at(rootfs: &Path) -> std::io::Result<()> {
    use nix::mount::mount;

    for (rel, src, fstype, flags, opts) in pseudo_mounts() {
        let path: PathBuf = rootfs.join(rel);
        let _ = std::fs::create_dir_all(&path);
        if let Err(err) = mount(Some(src), &path, Some(fstype), flags, opts) {
            // Optional inside single-UID user namespaces / non-TTY workloads.
            if rel == "dev/pts" || rel == "sys" {
                tracing::warn!(error = %err, "optional mount {rel} skipped");
                continue;
            }
            return Err(std::io::Error::other(format!(
                "mount {} failed: {err}",
                path.display()
            )));
        }
    }

    let dev = rootfs.join("dev");
    for name in &["null", "zero", "random", "urandom", "tty"] {
        let path = dev.join(name);
        if !path.exists() {
            let _ = std::fs::write(&path, []);
        }
    }

    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::path_is_proc_mount;
    use std::path::Path;

    #[test]
    fn path_is_proc_mount_recognizes_host_proc() {
        assert!(path_is_proc_mount(Path::new("/proc")));
    }
}
