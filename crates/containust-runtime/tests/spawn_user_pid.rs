//! Privileged spawn fixtures for user + PID namespaces (P11.1 / P11.2).

#![cfg(target_os = "linux")]

use std::path::PathBuf;

use containust_core::namespace::NamespaceConfig;
use containust_runtime::process::{ProcessConfig, spawn_container_process};

/// Builds a minimal rootfs with static busybox and spawns `busybox true`
/// under user+PID namespaces. Requires root / userns (CI `privileged-linux`).
#[test]
#[ignore = "requires root privileges, user namespaces, and busybox-static"]
fn spawn_with_user_and_pid_runs_true() {
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
    std::fs::copy(&busybox, &dst).expect("copy busybox");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o755)).expect("chmod");

    let config = ProcessConfig {
        command: vec!["/bin/busybox".into(), "true".into()],
        env: Vec::new(),
        rootfs: root.path().to_path_buf(),
        readonly_rootfs: false,
        volumes: Vec::new(),
        namespaces: NamespaceConfig::default().with_user_and_pid(),
        log_path: None,
    };
    let pid = spawn_container_process(&config).expect("spawn user+pid");
    assert!(pid > 0, "init host pid should be positive, got {pid}");

    let status = nix::sys::wait::waitpid(
        nix::unistd::Pid::from_raw(i32::try_from(pid).expect("pid fits i32")),
        None,
    )
    .expect("waitpid");
    assert!(
        matches!(status, nix::sys::wait::WaitStatus::Exited(_, 0)),
        "expected exit 0, got {status:?}"
    );
}
