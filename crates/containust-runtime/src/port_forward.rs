//! Userspace TCP port forwarder into a container network namespace.
//!
//! Listens on the host and relays into `127.0.0.1:<container>` after
//! `setns` into the target netns — no `CAP_NET_ADMIN` / nftables required.

#![cfg(target_os = "linux")]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};

use containust_common::error::{ContainustError, Result};
use containust_common::types::PortMapping;
use nix::sched::{CloneFlags, setns};
use nix::unistd::{ForkResult, fork};

/// Starts one forwarder process per port mapping.
///
/// # Errors
///
/// Returns an error when a listener cannot bind or fork fails.
pub fn start_forwarders(netns: &Path, mappings: &[PortMapping]) -> Result<Vec<u32>> {
    let mut pids = Vec::with_capacity(mappings.len());
    for mapping in mappings {
        pids.push(start_one(netns, *mapping)?);
    }
    Ok(pids)
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

fn start_one(netns: &Path, mapping: PortMapping) -> Result<u32> {
    let listener =
        TcpListener::bind(("127.0.0.1", mapping.host)).map_err(|source| ContainustError::Io {
            path: PathBuf::from(format!("127.0.0.1:{}", mapping.host)),
            source,
        })?;
    let netns = netns.to_path_buf();

    // SAFETY: forwarder is a dedicated single-threaded child.
    let fork_result = unsafe { fork() }.map_err(|e| ContainustError::Config {
        message: format!("fork port-forwarder failed: {e}"),
    })?;
    match fork_result {
        ForkResult::Parent { child } => Ok(u32::try_from(child.as_raw()).unwrap_or(u32::MAX)),
        ForkResult::Child => {
            forwarder_loop(listener, &netns, mapping);
            // SAFETY: forwarder never returns to parent.
            unsafe { libc::_exit(0) };
        }
    }
}

fn forwarder_loop(listener: TcpListener, netns: &Path, mapping: PortMapping) {
    for incoming in listener.incoming() {
        let Ok(client) = incoming else {
            continue;
        };
        // SAFETY: per-connection child; setns requires single-threaded.
        match unsafe { fork() } {
            Ok(ForkResult::Parent { .. }) => {}
            Ok(ForkResult::Child) => {
                let _ = relay_one(client, netns, mapping.container);
                // SAFETY: connection handler exit.
                unsafe { libc::_exit(0) };
            }
            Err(_) => {}
        }
    }
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
    let _ = to_up.join();
    Ok(())
}
