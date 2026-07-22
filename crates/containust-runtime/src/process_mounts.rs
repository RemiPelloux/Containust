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
pub(crate) fn mount_pseudo_filesystems() -> std::io::Result<()> {
    use nix::mount::mount;

    for (path, src, fstype, flags, opts) in pseudo_mounts() {
        let _ = std::fs::create_dir_all(path);
        mount(Some(src), path, Some(fstype), flags, opts)
            .map_err(|e| std::io::Error::other(format!("mount {path} failed: {e}")))?;
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
