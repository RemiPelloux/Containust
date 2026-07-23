//! Linux shared-netns helpers (no `CAP_NET_ADMIN` required).

use std::ffi::CString;
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};

use containust_common::error::{ContainustError, Result};
use nix::sched::{CloneFlags, setns, unshare};
use nix::unistd::{ForkResult, fork};

/// Returns the persistent netns bind path for a project network.
#[must_use]
pub fn network_ns_path(data_dir: &Path, network: &str) -> PathBuf {
    data_dir.join("networks").join(network).join("ns")
}

/// Ensures a shared network namespace exists and has loopback up.
///
/// # Errors
///
/// Returns an error when the netns cannot be created or persisted.
pub fn ensure_shared_netns(data_dir: &Path, network: &str) -> Result<PathBuf> {
    let path = network_ns_path(data_dir, network);
    if path.exists() {
        return Ok(path);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ContainustError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let _ns_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(|source| ContainustError::Io {
            path: path.clone(),
            source,
        })?;

    // SAFETY: child never returns into the parent Rust stack.
    let fork_result = unsafe { fork() }.map_err(|e| ContainustError::Config {
        message: format!("fork for shared netns failed: {e}"),
    })?;
    match fork_result {
        ForkResult::Parent { child } => {
            let status =
                nix::sys::wait::waitpid(child, None).map_err(|e| ContainustError::Config {
                    message: format!("wait shared netns child failed: {e}"),
                })?;
            if !matches!(status, nix::sys::wait::WaitStatus::Exited(_, 0)) {
                return Err(ContainustError::Config {
                    message: format!("shared netns setup failed: {status:?}"),
                });
            }
            Ok(path)
        }
        ForkResult::Child => {
            let code = setup_and_persist_netns(&path);
            // SAFETY: intentional child exit.
            unsafe { libc::_exit(code) };
        }
    }
}

fn setup_and_persist_netns(path: &Path) -> i32 {
    if unshare(CloneFlags::CLONE_NEWNET).is_err() {
        return 1;
    }
    if ensure_loopback().is_err() {
        return 2;
    }
    let Ok(src) = CString::new("/proc/self/ns/net") else {
        return 3;
    };
    let Ok(dst) = CString::new(path.to_string_lossy().as_bytes()) else {
        return 3;
    };
    // SAFETY: NUL-terminated paths; MS_BIND persists the netns inode.
    let rc = unsafe {
        libc::mount(
            src.as_ptr(),
            dst.as_ptr(),
            std::ptr::null(),
            libc::MS_BIND,
            std::ptr::null(),
        )
    };
    if rc == 0 { 0 } else { 4 }
}

/// Brings up the loopback interface in the current network namespace.
///
/// # Errors
///
/// Returns an error when `ip link set lo up` fails.
pub fn ensure_loopback() -> Result<()> {
    let status = std::process::Command::new("ip")
        .args(["link", "set", "lo", "up"])
        .status();
    match status {
        Ok(code) if code.success() => Ok(()),
        Ok(code) => Err(ContainustError::Config {
            message: format!("`ip link set lo up` failed with {code}"),
        }),
        Err(source) => Err(ContainustError::Io {
            path: PathBuf::from("ip"),
            source,
        }),
    }
}

/// Joins an existing network namespace by path.
///
/// # Errors
///
/// Returns an error when the netns cannot be opened or joined.
pub fn join_netns(path: &Path) -> Result<()> {
    let file = std::fs::File::open(path).map_err(|source| ContainustError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    setns(file.as_fd(), CloneFlags::CLONE_NEWNET).map_err(|e| {
        ContainustError::PermissionDenied {
            message: format!("setns({}) failed: {e}", path.display()),
        }
    })?;
    Ok(())
}

/// Writes `/etc/hosts` so CONNECT peer names resolve to loopback.
///
/// # Errors
///
/// Returns an error when the hosts file cannot be written.
pub fn write_container_hosts(rootfs: &Path, names: &[String]) -> Result<()> {
    let etc = rootfs.join("etc");
    std::fs::create_dir_all(&etc).map_err(|source| ContainustError::Io {
        path: etc.clone(),
        source,
    })?;
    let mut body = String::from("127.0.0.1\tlocalhost\n");
    for name in names {
        if name != "localhost" {
            let _ = writeln!(body, "127.0.0.1\t{name}");
        }
    }
    let hosts = etc.join("hosts");
    std::fs::write(&hosts, body).map_err(|source| ContainustError::Io {
        path: hosts,
        source,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn write_container_hosts_maps_peers_to_loopback() {
        let dir = tempfile::tempdir().expect("tempdir");
        let rootfs = dir.path().join("rootfs");
        write_container_hosts(&rootfs, &["api".into(), "db".into(), "localhost".into()])
            .expect("hosts");
        let body = std::fs::read_to_string(rootfs.join("etc/hosts")).expect("read");
        assert!(body.contains("127.0.0.1\tlocalhost"));
        assert!(body.contains("127.0.0.1\tapi"));
        assert!(body.contains("127.0.0.1\tdb"));
        assert_eq!(body.matches("127.0.0.1\tlocalhost").count(), 1);
    }
}
