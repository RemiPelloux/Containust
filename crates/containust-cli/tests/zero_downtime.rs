//! Sprint 13 exit gate (P13.7): a `start-first` update replaces a component
//! while a client keeps requesting its published port, with zero failed
//! requests; an unhealthy replacement leaves the old container serving.
//!
//! Requires root, user namespaces, cgroups v2, and `busybox-static` (for
//! `httpd` and `wget` inside the fixture rootfs), so it is `#[ignore]` and
//! runs in the privileged Linux CI job.

#![cfg(target_os = "linux")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const HOST_PORT: u16 = 18_080;

fn ctst(dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ctst"));
    let _ = command.current_dir(dir);
    command
}

fn run_checked(command: &mut Command) -> Output {
    let output = command.output().expect("spawn ctst");
    assert!(
        output.status.success(),
        "command failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// Builds a busybox rootfs whose web root serves `body`.
fn build_rootfs(dir: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;

    let busybox = ["/bin/busybox", "/usr/bin/busybox"]
        .iter()
        .map(Path::new)
        .find(|p| p.exists())
        .expect("install busybox-static to run the zero-downtime gate");
    std::fs::create_dir_all(dir.join("bin")).expect("mkdir bin");
    std::fs::create_dir_all(dir.join("www")).expect("mkdir www");
    let dst = dir.join("bin/busybox");
    let _ = std::fs::copy(busybox, &dst).expect("copy busybox");
    std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    std::fs::write(dir.join("www/index.html"), body).expect("write index");
}

fn composition(rootfs: &Path, probe: &str) -> String {
    format!(
        r#"COMPONENT web {{
    image = "file://{rootfs}"
    command = ["/bin/busybox", "httpd", "-f", "-p", "8080", "-h", "/www"]
    network = "none"
    ports = [8080]
    update = "start-first"
    stop_grace = "1s"
    healthcheck = {{
        command = {probe}
        interval = "1s"
        retries = 3
        start_period = "5s"
    }}
}}

EXPOSE {HOST_PORT}:8080
"#,
        rootfs = rootfs.display()
    )
}

const HTTP_PROBE: &str =
    r#"["/bin/busybox", "wget", "-q", "-O", "/dev/null", "http://127.0.0.1:8080/"]"#;
const FAILING_PROBE: &str = r#"["/bin/busybox", "false"]"#;

/// One HTTP/1.0 request against the published port; returns the body.
fn fetch() -> Result<String, String> {
    let mut stream =
        TcpStream::connect(("127.0.0.1", HOST_PORT)).map_err(|e| format!("connect: {e}"))?;
    let timeout = Some(Duration::from_secs(5));
    stream
        .set_read_timeout(timeout)
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
        .filter(|(head, _)| head.starts_with("HTTP/1.") && head.contains(" 200 "))
        .map(|(_, body)| body.trim().to_string())
        .ok_or_else(|| format!("bad response: {response:?}"))
}

fn wait_for_body(expected: &str) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match fetch() {
            Ok(body) if body == expected => return,
            other if Instant::now() > deadline => {
                panic!("never served {expected:?}; last: {other:?}")
            }
            _ => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

struct Project {
    workspace: tempfile::TempDir,
    file: PathBuf,
}

impl Project {
    fn deploy_v1() -> Self {
        let workspace = tempfile::tempdir().expect("tempdir");
        build_rootfs(&workspace.path().join("v1"), "v1");
        build_rootfs(&workspace.path().join("v2"), "v2");
        let file = workspace.path().join("app.ctst");
        let project = Self { workspace, file };
        project.write("v1", HTTP_PROBE);
        let _ = run_checked(project.ctst().arg("run").arg("--detach").arg(&project.file));
        wait_for_body("v1");
        project
    }

    fn ctst(&self) -> Command {
        ctst(self.workspace.path())
    }

    fn write(&self, version: &str, probe: &str) {
        let rootfs = self.workspace.path().join(version);
        std::fs::write(&self.file, composition(&rootfs, probe)).expect("write ctst");
    }

    fn container_names(&self) -> String {
        let ps = run_checked(self.ctst().arg("ps").arg("--all"));
        String::from_utf8_lossy(&ps.stdout).into_owned()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = self.ctst().arg("stop").arg("--force").output();
        let _ = self.ctst().arg("rm").arg("web").output();
    }
}

/// Requests the published port in a loop until `stop` is set.
fn spawn_client(stop: Arc<AtomicBool>) -> std::thread::JoinHandle<Vec<Result<String, String>>> {
    std::thread::spawn(move || {
        let mut results = Vec::new();
        while !stop.load(Ordering::Acquire) {
            results.push(fetch());
            std::thread::sleep(Duration::from_millis(20));
        }
        results
    })
}

#[test]
#[ignore = "requires root privileges, user namespaces, cgroups v2, and busybox-static"]
fn start_first_update_drops_no_requests() {
    let project = Project::deploy_v1();
    let stop = Arc::new(AtomicBool::new(false));
    let client = spawn_client(Arc::clone(&stop));

    project.write("v2", HTTP_PROBE);
    let update = run_checked(project.ctst().arg("run").arg("--detach").arg(&project.file));
    let stderr = String::from_utf8_lossy(&update.stderr);
    assert!(stderr.contains("start-first"), "stderr: {stderr}");
    wait_for_body("v2");
    std::thread::sleep(Duration::from_millis(500));
    stop.store(true, Ordering::Release);
    let results = client.join().expect("client thread");

    let failures: Vec<_> = results.iter().filter_map(|r| r.as_ref().err()).collect();
    assert!(
        failures.is_empty(),
        "{} of {} requests failed during the update: {failures:?}",
        failures.len(),
        results.len()
    );
    let bodies: Vec<&str> = results.iter().flatten().map(String::as_str).collect();
    let first_v2 = bodies.iter().position(|b| *b == "v2").expect("served v2");
    assert!(first_v2 > 0 && bodies[0] == "v1", "bodies: {bodies:?}");
    assert!(
        bodies[first_v2..].iter().all(|b| *b == "v2"),
        "v1 served after the switch: {bodies:?}"
    );
    let ps = project.container_names();
    assert!(
        !ps.contains("web~next"),
        "staged container left behind: {ps}"
    );
}

#[test]
#[ignore = "requires root privileges, user namespaces, cgroups v2, and busybox-static"]
fn unhealthy_replacement_keeps_old_container_serving() {
    let project = Project::deploy_v1();

    project.write("v2", FAILING_PROBE);
    let update = project
        .ctst()
        .arg("run")
        .arg("--detach")
        .arg(&project.file)
        .output()
        .expect("spawn ctst run");
    assert!(
        !update.status.success(),
        "update with a failing probe must fail"
    );
    let stderr = String::from_utf8_lossy(&update.stderr);
    assert!(stderr.contains("never became healthy"), "stderr: {stderr}");

    assert_eq!(fetch().as_deref(), Ok("v1"));
    let ps = project.container_names();
    assert!(
        !ps.contains("web~next"),
        "staged container left behind: {ps}"
    );
}
