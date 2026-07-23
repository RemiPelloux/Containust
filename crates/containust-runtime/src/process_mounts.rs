//! Pseudo-filesystem mounts applied after `pivot_root`.

#![allow(missing_docs)]

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
        // Avoid `gid=5` — unmapped in single-UID user namespaces (range 1).
        (
            "/dev/pts",
            "devpts",
            "devpts",
            MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC,
            Some("newinstance,ptmxmode=0666,mode=0620"),
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
pub fn mount_pseudo_filesystems() -> std::io::Result<()> {
    use nix::mount::mount;

    for (path, src, fstype, flags, opts) in pseudo_mounts() {
        let _ = std::fs::create_dir_all(path);
        if let Err(err) = mount(Some(src), path, Some(fstype), flags, opts) {
            // Optional inside single-UID user namespaces / non-TTY workloads.
            if path == "/dev/pts" || path == "/sys" {
                tracing::warn!(error = %err, "optional mount {path} skipped");
                continue;
            }
            return Err(std::io::Error::other(format!("mount {path} failed: {err}")));
        }
    }

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
