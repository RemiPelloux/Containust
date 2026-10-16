//! Port forwarder fixture: relays a host port into a server running in a
//! separate, persisted network namespace, and switches targets without
//! unbinding the port (start-first handoff).
//!
//! Needs `CAP_SYS_ADMIN` (root, or `unshare -rnm` for a rootless run),
//! `unshare`, `ip`, and `python3`, so it is `#[ignore]`.

#![cfg(target_os = "linux")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use containust_common::types::PortMapping;
use containust_runtime::port_forward;

/// An HTTP server serving `body` inside its own netns, persisted at `ns`.
struct NetnsServer {
    child: Child,
    ns: PathBuf,
}

impl NetnsServer {
    fn start(dir: &Path, name: &str, body: &str) -> Self {
        let www = dir.join(format!("{name}-www"));
        std::fs::create_dir_all(&www).expect("www");
        std::fs::write(www.join("index.html"), body).expect("index");
        let ns = dir.join(format!("{name}-ns"));
        std::fs::write(&ns, b"").expect("ns file");
        let script = format!(
            "ip link set lo up && exec python3 -m http.server 8080 --bind 127.0.0.1 -d {}",
            www.display()
        );
        let child = Command::new("unshare")
            .arg(format!("--net={}", ns.display()))
            .args(["sh", "-c", &script])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn unshare (needs CAP_SYS_ADMIN)");
        Self { child, ns }
    }
}

impl Drop for NetnsServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = nix::mount::umount2(&self.ns, nix::mount::MntFlags::MNT_DETACH);
    }
}

fn fetch(port: u16) -> Result<String, String> {
    let mut stream =
        TcpStream::connect(("127.0.0.1", port)).map_err(|e| format!("connect: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| e.to_string())?;
    stream
        .write_all(b"GET /index.html HTTP/1.0\r\n\r\n")
        .map_err(|e| format!("write: {e}"))?;
    let mut response = String::new();
    let _ = stream
        .read_to_string(&mut response)
        .map_err(|e| format!("read: {e}"))?;
    response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.trim().to_string())
        .ok_or_else(|| format!("bad response: {response:?}"))
}

fn wait_for(port: u16, expected: &str) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match fetch(port) {
            Ok(body) if body == expected => return,
            other if Instant::now() > deadline => panic!("never got {expected:?}: {other:?}"),
            _ => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

#[test]
#[ignore = "requires CAP_SYS_ADMIN (root or unshare -rnm), unshare, ip, python3"]
fn forwarder_relays_into_netns_and_retargets_without_unbinding() {
    let dir = tempfile::tempdir().expect("tempdir");
    let data = dir.path().join("data");
    let blue = NetnsServer::start(dir.path(), "blue", "blue");
    let green = NetnsServer::start(dir.path(), "green", "green");
    std::thread::sleep(Duration::from_millis(500));

    let mapping = PortMapping {
        host: 18_090,
        container: 8080,
    };
    let pids = port_forward::start_forwarders(&data, &blue.ns, &[mapping]).expect("start");
    wait_for(mapping.host, "blue");

    port_forward::retarget(&data, &green.ns, &[mapping]).expect("retarget");
    wait_for(mapping.host, "green");
    assert!(port_forward::forwarders_alive(&pids));

    port_forward::stop_forwarders_and_wait(&pids);
    assert!(
        TcpStream::connect(("127.0.0.1", mapping.host)).is_err(),
        "host port must be released once the forwarder stops"
    );
}
