//! Userspace TCP port forwarder into a container network namespace.
//!
//! Listens on the host and relays into `127.0.0.1:<container>` after
//! `setns` into the target netns — no `CAP_NET_ADMIN` / nftables required.
//!
//! Each forwarder resolves its target through a per-host-port symlink
//! (`<data_dir>/ports/<host>`) for every new connection. Repointing that
//! link moves new connections to another container without unbinding the
//! port, which is how start-first updates switch traffic with no gap.

#![cfg(target_os = "linux")]

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::os::fd::{AsFd, AsRawFd};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use containust_common::error::{ContainustError, Result};
use containust_common::types::PortMapping;
use nix::sched::{CloneFlags, setns};
use nix::unistd::{ForkResult, fork};

/// How long to wait for stopped forwarders to release their ports.
const FORWARDER_EXIT_WAIT: Duration = Duration::from_secs(2);

/// Stable path a host port's forwarder resolves for every connection.
#[must_use]
pub fn target_link(data_dir: &Path, host_port: u16) -> PathBuf {
    data_dir.join("ports").join(host_port.to_string())
}

/// Atomically points `host_port`'s target link at `netns`.
///
/// # Errors
///
/// Returns an error when the link cannot be created or renamed into place.
pub fn point_target(data_dir: &Path, host_port: u16, netns: &Path) -> Result<()> {
    let io_err = |path: &Path| {
        let path = path.to_path_buf();
        move |source| ContainustError::Io { path, source }
    };
    let link = target_link(data_dir, host_port);
    let dir = data_dir.join("ports");
    std::fs::create_dir_all(&dir).map_err(io_err(&dir))?;
    // The link is resolved from inside forwarders whose cwd may differ.
    let netns = std::fs::canonicalize(netns).map_err(io_err(netns))?;
    let tmp = dir.join(format!(".{host_port}.{}.tmp", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    std::os::unix::fs::symlink(&netns, &tmp).map_err(io_err(&tmp))?;
    std::fs::rename(&tmp, &link).map_err(io_err(&link))
}

/// Whether every mapping already has a target link (forwarders started by
/// a runtime that supports retargeting).
#[must_use]
pub fn targets_exist(data_dir: &Path, mappings: &[PortMapping]) -> bool {
    mappings.iter().all(|mapping| {
        std::fs::symlink_metadata(target_link(data_dir, mapping.host))
            .is_ok_and(|meta| meta.file_type().is_symlink())
    })
}

/// Starts one forwarder process per port mapping, targeting `netns`.
///
/// Every port is bound before any target link changes, so a port that is
/// already taken (by another container's forwarder, say) fails the call
/// without redirecting that port's existing traffic.
///
/// # Errors
///
/// Returns an error when a listener cannot bind, a link cannot be
/// written, or fork fails.
pub fn start_forwarders(
    data_dir: &Path,
    netns: &Path,
    mappings: &[PortMapping],
) -> Result<Vec<u32>> {
    let listeners = mappings
        .iter()
        .map(|mapping| bind(mapping.host))
        .collect::<Result<Vec<_>>>()?;
    let mut pids = Vec::with_capacity(mappings.len());
    for (mapping, listener) in mappings.iter().zip(listeners) {
        let started = point_target(data_dir, mapping.host, netns).and_then(|()| {
            spawn_forwarder(listener, &target_link(data_dir, mapping.host), *mapping)
        });
        match started {
            Ok(pid) => pids.push(pid),
            Err(error) => {
                stop_forwarders_and_wait(&pids);
                return Err(error);
            }
        }
    }
    Ok(pids)
}

/// Repoints running forwarders at `netns`: new connections go there,
/// connections already relayed finish where they are.
///
/// # Errors
///
/// Returns an error when a target link cannot be updated.
pub fn retarget(data_dir: &Path, netns: &Path, mappings: &[PortMapping]) -> Result<()> {
    for mapping in mappings {
        point_target(data_dir, mapping.host, netns)?;
    }
    Ok(())
}

/// Whether every forwarder process is still alive.
#[must_use]
pub fn forwarders_alive(pids: &[u32]) -> bool {
    !pids.is_empty() && pids.iter().all(|&pid| pid_alive(pid))
}

/// Stops forwarder processes (best-effort).
pub fn stop_forwarders(pids: &[u32]) {
    for &pid in pids {
        let Ok(raw) = i32::try_from(pid) else {
            continue;
        };
        let _ = nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(raw),
            nix::sys::signal::Signal::SIGTERM,
        );
    }
}

/// Stops forwarders and waits (bounded) until they exit, so their host
/// ports can be bound again immediately.
pub fn stop_forwarders_and_wait(pids: &[u32]) {
    stop_forwarders(pids);
    let deadline = Instant::now() + FORWARDER_EXIT_WAIT;
    while pids.iter().any(|&pid| pid_alive(pid)) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Liveness that is not fooled by zombies: a forwarder that is our own
/// child (long-lived SDK hosts) is reaped here; anything else is probed.
fn pid_alive(pid: u32) -> bool {
    use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};

    let Ok(raw) = i32::try_from(pid) else {
        return false;
    };
    let pid = nix::unistd::Pid::from_raw(raw);
    match waitpid(pid, Some(WaitPidFlag::WNOHANG)) {
        Ok(WaitStatus::StillAlive) => true,
        Ok(_) => false,
        Err(_) => nix::sys::signal::kill(pid, None).is_ok(),
    }
}

fn bind(host_port: u16) -> Result<TcpListener> {
    TcpListener::bind(("127.0.0.1", host_port)).map_err(|source| ContainustError::Io {
        path: PathBuf::from(format!("127.0.0.1:{host_port}")),
        source,
    })
}

fn spawn_forwarder(listener: TcpListener, target: &Path, mapping: PortMapping) -> Result<u32> {
    let target = target.to_path_buf();
    // SAFETY: the child only uses fork-safe primitives before it settles
    // into its own single-threaded accept loop.
    let fork_result = unsafe { fork() }.map_err(|e| ContainustError::Config {
        message: format!("fork port-forwarder failed: {e}"),
    })?;
    match fork_result {
        ForkResult::Parent { child } => Ok(u32::try_from(child.as_raw()).unwrap_or(u32::MAX)),
        ForkResult::Child => {
            let listener = keep_only_listener(listener);
            detach_from_caller();
            forwarder_loop(&listener, &target, mapping);
            // SAFETY: forwarder never returns to parent.
            unsafe { libc::_exit(0) };
        }
    }
}

/// Moves the listener to fd 3 and closes every other inherited descriptor
/// (state lock, log files, sockets of the `ctst` that forked us), so a
/// long-lived forwarder never pins resources of its parent.
fn keep_only_listener(listener: TcpListener) -> TcpListener {
    use std::os::fd::{FromRawFd, IntoRawFd};

    const LISTENER_FD: i32 = 3;
    let raw = listener.into_raw_fd();
    // SAFETY: plain descriptor shuffling in this single-threaded child.
    unsafe {
        if raw != LISTENER_FD {
            let _ = libc::dup2(raw, LISTENER_FD);
            let _ = libc::close(raw);
        }
        let first = (LISTENER_FD + 1).unsigned_abs();
        if libc::syscall(libc::SYS_close_range, first, u32::MAX, 0_u32) != 0 {
            for fd in LISTENER_FD + 1..4096 {
                let _ = libc::close(fd);
            }
        }
        TcpListener::from_raw_fd(LISTENER_FD)
    }
}

/// Makes the forwarder independent of the `ctst` invocation that forked it:
/// a new session (no SIGHUP when the terminal closes) and stdio on
/// `/dev/null`, so callers capturing `ctst run -d` output see EOF.
fn detach_from_caller() {
    let _ = nix::unistd::setsid();
    // SAFETY: open/dup2/close on this single-threaded child's descriptors.
    unsafe {
        let null = libc::open(c"/dev/null".as_ptr(), libc::O_RDWR);
        if null >= 0 {
            for fd in 0..=2 {
                if fd != null {
                    let _ = libc::dup2(null, fd);
                }
            }
            if null > 2 {
                let _ = libc::close(null);
            }
        }
    }
}

/// Most connections one forwarder relays at once; further clients wait in
/// the listen backlog until a slot frees up.
const MAX_CONNECTIONS: usize = 512;

fn forwarder_loop(listener: &TcpListener, target: &Path, mapping: PortMapping) {
    let mut active = 0_usize;
    for incoming in listener.incoming() {
        let Ok(client) = incoming else {
            continue;
        };
        active = active.saturating_sub(reap_children(false));
        while active >= MAX_CONNECTIONS {
            active = active.saturating_sub(reap_children(true).max(1));
        }
        // SAFETY: per-connection child; setns requires single-threaded.
        match unsafe { fork() } {
            Ok(ForkResult::Child) => {
                // Release the listener so stopping the forwarder frees the
                // host port even while connections are open.
                // SAFETY: the child never touches the listener again.
                let _ = unsafe { libc::close(listener.as_raw_fd()) };
                let _ = relay_one(client, target, mapping.container);
                // SAFETY: connection handler exit.
                unsafe { libc::_exit(0) };
            }
            Ok(ForkResult::Parent { .. }) => active += 1,
            Err(_) => {}
        }
    }
}

/// Reaps finished connection handlers; with `block`, waits for at least one.
fn reap_children(block: bool) -> usize {
    use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};

    let mut reaped = 0;
    let mut flags = (!block).then_some(WaitPidFlag::WNOHANG);
    while let Ok(status) = waitpid(None, flags) {
        if status == WaitStatus::StillAlive {
            break;
        }
        reaped += 1;
        flags = Some(WaitPidFlag::WNOHANG);
    }
    reaped
}

fn relay_one(client: TcpStream, netns: &Path, container_port: u16) -> std::io::Result<()> {
    let ns = std::fs::File::open(netns)?;
    setns(ns.as_fd(), CloneFlags::CLONE_NEWNET)
        .map_err(|e| std::io::Error::other(format!("setns: {e}")))?;
    let upstream = TcpStream::connect(("127.0.0.1", container_port))?;
    let mut client_read = client.try_clone()?;
    let mut upstream_write = upstream.try_clone()?;
    let mut upstream_read = upstream;
    let mut client_write = client;
    let to_up = std::thread::spawn(move || {
        let mut buf = [0_u8; 8192];
        while let Ok(n) = client_read.read(&mut buf) {
            if n == 0 {
                break;
            }
            if upstream_write.write_all(&buf[..n]).is_err() {
                break;
            }
        }
        let _ = upstream_write.shutdown(Shutdown::Write);
    });
    let mut buf = [0_u8; 8192];
    while let Ok(n) = upstream_read.read(&mut buf) {
        if n == 0 {
            break;
        }
        if client_write.write_all(&buf[..n]).is_err() {
            break;
        }
    }
    // Propagate the server's close: clients reading until EOF (HTTP/1.0,
    // close-delimited protocols) would otherwise wait on us forever.
    let _ = client_write.shutdown(Shutdown::Write);
    let _ = to_up.join();
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn point_target_swaps_link_atomically() {
        let dir = tempfile::tempdir().expect("tempdir");
        let first = dir.path().join("a-ns");
        let second = dir.path().join("b-ns");
        std::fs::write(&first, b"").expect("first");
        std::fs::write(&second, b"").expect("second");
        let mapping = PortMapping::identity(18080);
        assert!(!targets_exist(dir.path(), &[mapping]));

        point_target(dir.path(), 18080, &first).expect("point first");
        assert!(targets_exist(dir.path(), &[mapping]));
        let link = target_link(dir.path(), 18080);
        assert_eq!(
            std::fs::read_link(&link).expect("link"),
            std::fs::canonicalize(&first).expect("canon")
        );

        retarget(dir.path(), &second, &[mapping]).expect("retarget");
        assert_eq!(
            std::fs::read_link(&link).expect("link"),
            std::fs::canonicalize(&second).expect("canon")
        );
        let leftovers: Vec<_> = std::fs::read_dir(dir.path().join("ports"))
            .expect("ports dir")
            .filter_map(std::result::Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "no temp links left behind");
    }

    #[test]
    fn forwarders_alive_requires_live_pids() {
        assert!(!forwarders_alive(&[]));
        assert!(forwarders_alive(&[std::process::id()]));
        assert!(!forwarders_alive(&[std::process::id(), u32::MAX]));
    }

    #[test]
    fn point_target_rejects_missing_netns() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(point_target(dir.path(), 18081, &dir.path().join("missing")).is_err());
    }
}
