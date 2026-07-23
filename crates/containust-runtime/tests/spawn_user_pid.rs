//! Privileged spawn fixtures for user + PID namespaces (P11.1 / P11.2).

#![cfg(target_os = "linux")]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::Duration;

use containust_core::namespace::NamespaceConfig;
use containust_runtime::process::{ProcessConfig, spawn_container_process};
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

/// Spawns `busybox sleep` under user+PID namespaces and verifies the host
/// PID is alive (double-fork reparents init, so waitpid is not used).
#[test]
#[ignore = "requires root privileges, user namespaces, and busybox-static"]
fn spawn_with_user_and_pid_runs_sleep() {
    let root = tempfile::tempdir().expect("tempdir");
    let bin = root.path().join("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    std::fs::create_dir_all(root.path().join(".old_root")).expect("old_root");

    let busybox = ["/bin/busybox", "/usr/bin/busybox"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
        .expect("need busybox-static (apt install busybox-static)");
    let dst = bin.join("busybox");
    let _ = std::fs::copy(&busybox, &dst).expect("copy busybox");
    std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o755)).expect("chmod");

    let config = ProcessConfig {
        command: vec!["/bin/busybox".into(), "sleep".into(), "30".into()],
        env: Vec::new(),
        rootfs: root.path().to_path_buf(),
        readonly_rootfs: false,
        volumes: Vec::new(),
        namespaces: NamespaceConfig::default().with_user_and_pid(),
        join_netns: None,
        log_path: None,
    };
    let pid = spawn_container_process(&config).expect("spawn user+pid");
    assert!(pid > 0, "init host pid should be positive, got {pid}");

    let nix_pid = Pid::from_raw(i32::try_from(pid).expect("pid fits i32"));
    // Signal 0 probes liveness; the test process is not the parent after
    // the PID-namespace double-fork, so waitpid would return ECHILD.
    kill(nix_pid, None).expect("container init should be alive");
    kill(nix_pid, Signal::SIGKILL).expect("kill container init");
    std::thread::sleep(Duration::from_millis(50));
    assert!(
        kill(nix_pid, None).is_err(),
        "container init should be gone after SIGKILL"
    );
}
