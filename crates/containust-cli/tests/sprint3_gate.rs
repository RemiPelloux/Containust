//! Sprint 3 exit gate: import an image once, verify it by digest, copy
//! the content-addressed store into an air-gapped project, and use it
//! with `--offline` without network access or the original source.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn ctst() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ctst"))
}

fn run_checked(command: &mut Command) -> Output {
    let output = command.output().expect("failed to spawn ctst");
    assert!(
        output.status.success(),
        "command failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn build_rootfs(root: &Path) {
    std::fs::create_dir_all(root.join("bin")).expect("mkdir bin");
    std::fs::write(root.join("bin/app"), b"#!/bin/sh\necho containust\n").expect("write app");
    std::fs::write(root.join("release.txt"), b"sprint-3\n").expect("write marker");
}

fn copy_dir(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).expect("mkdir destination");
    for entry in std::fs::read_dir(source).expect("read source") {
        let entry = entry.expect("dir entry");
        let target = destination.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            let _ = std::fs::copy(entry.path(), &target).expect("copy file");
        }
    }
}

fn imported_digest(project_dir: &Path, name: &str) -> String {
    let catalog = containust_image::registry::ImageCatalog::open(project_dir).expect("catalog");
    let entry = catalog.find(name).expect("imported image present");
    entry.digest.expect("digest recorded")
}

struct Gate {
    _workspace: tempfile::TempDir,
    online_ctst: PathBuf,
    airgap_dir: PathBuf,
    airgap_ctst: PathBuf,
    digest: String,
}

/// Imports the fixture image online, then builds an air-gapped copy of
/// the store with the original source deleted.
fn prepare_airgapped_project() -> Gate {
    let workspace = tempfile::tempdir().expect("tempdir");
    let online_dir = workspace.path().join("online");
    let rootfs = online_dir.join("rootfs");
    build_rootfs(&rootfs);
    let online_ctst = online_dir.join("app.ctst");
    std::fs::write(
        &online_ctst,
        format!(
            "COMPONENT app {{\n    image = \"file://{}\"\n}}\n",
            rootfs.display()
        ),
    )
    .expect("write online ctst");

    let _ = run_checked(ctst().arg("build").arg(&online_ctst));
    let online_project = containust_common::constants::project_dir(&online_ctst);
    let digest = imported_digest(&online_project, "app");

    // Air-gapped project: only the content-addressed store travels.
    let airgap_dir = workspace.path().join("airgap");
    std::fs::create_dir_all(&airgap_dir).expect("mkdir airgap");
    let airgap_project = airgap_dir.join(".containust");
    copy_dir(
        &online_project.join("images"),
        &airgap_project.join("images"),
    );
    copy_dir(
        &online_project.join("layers"),
        &airgap_project.join("layers"),
    );
    let airgap_ctst = airgap_dir.join("app.ctst");
    std::fs::write(
        &airgap_ctst,
        format!("COMPONENT app {{\n    image = \"image://app@sha256:{digest}\"\n}}\n"),
    )
    .expect("write airgap ctst");

    // The original source must no longer be needed anywhere.
    std::fs::remove_dir_all(&online_dir).expect("remove online project");

    Gate {
        _workspace: workspace,
        online_ctst,
        airgap_dir,
        airgap_ctst,
        digest,
    }
}

#[test]
fn gate_import_is_digest_verified_and_usable_offline() {
    let gate = prepare_airgapped_project();
    assert!(!gate.online_ctst.exists(), "online source removed");

    // Offline validation of the air-gapped composition succeeds.
    let output = run_checked(
        ctst()
            .arg("--offline")
            .arg("build")
            .arg(&gate.airgap_ctst)
            .env("CONTAINUST_OFFLINE", "1"),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Already imported"), "stdout: {stdout}");
    assert!(stdout.contains(&gate.digest), "stdout: {stdout}");

    // The rootfs is reconstructed from the local store alone, and the
    // pinned digest is enforced.
    let airgap_project = containust_common::constants::project_dir(&gate.airgap_ctst);
    let reference = containust_image::reference::ImageReference::parse(&format!(
        "image://app@sha256:{}",
        gate.digest
    ))
    .expect("parse reference");
    let target = gate.airgap_dir.join("materialized");
    containust_image::import::materialize_image(&airgap_project, &reference, &target)
        .expect("materialize offline");
    let app = std::fs::read(target.join("bin/app")).expect("read app");
    assert_eq!(app, b"#!/bin/sh\necho containust\n");
}

#[test]
fn gate_offline_plan_accepts_catalog_reference() {
    let gate = prepare_airgapped_project();
    let output = run_checked(ctst().arg("--offline").arg("plan").arg(&gate.airgap_ctst));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("image://app"), "stdout: {stdout}");
}

#[test]
fn gate_offline_build_rejects_remote_reference() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let ctst_file = workspace.path().join("remote.ctst");
    let digest = "0".repeat(64);
    std::fs::write(
        &ctst_file,
        format!(
            "COMPONENT app {{\n    image = \"https://example.test/app.tar@sha256:{digest}\"\n}}\n"
        ),
    )
    .expect("write ctst");

    let output = ctst()
        .arg("--offline")
        .arg("build")
        .arg(&ctst_file)
        .output()
        .expect("spawn ctst");

    assert!(!output.status.success(), "offline remote build must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("offline"), "stderr: {stderr}");
}

#[test]
fn gate_dry_run_writes_nothing() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let rootfs = workspace.path().join("rootfs");
    build_rootfs(&rootfs);
    let ctst_file = workspace.path().join("app.ctst");
    std::fs::write(
        &ctst_file,
        format!(
            "COMPONENT app {{\n    image = \"file://{}\"\n}}\n",
            rootfs.display()
        ),
    )
    .expect("write ctst");

    let output = run_checked(ctst().arg("build").arg("--dry-run").arg(&ctst_file));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Would import"), "stdout: {stdout}");

    let project_dir = containust_common::constants::project_dir(&ctst_file);
    assert!(
        !project_dir.join("images").join("catalog.json").exists(),
        "dry run must not write the catalog"
    );
}

#[cfg(target_os = "linux")]
fn write_busybox_ctst(path: &Path, image: &str) {
    std::fs::write(
        path,
        format!(
            "COMPONENT app {{\n    image = \"{image}\"\n    \
             command = [\"/bin/busybox\", \"sleep\", \"30\"]\n}}\n"
        ),
    )
    .expect("write ctst");
}

#[cfg(target_os = "linux")]
fn import_busybox_online(online_dir: &Path) -> (PathBuf, String) {
    use std::os::unix::fs::PermissionsExt;

    let busybox = ["/bin/busybox", "/usr/bin/busybox"]
        .iter()
        .map(Path::new)
        .find(|p| p.exists())
        .expect("install busybox-static to run the privileged gate fixture");
    let rootfs = online_dir.join("rootfs");
    std::fs::create_dir_all(rootfs.join("bin")).expect("mkdir bin");
    let dst = rootfs.join("bin/busybox");
    let _ = std::fs::copy(busybox, &dst).expect("copy busybox");
    std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o755)).expect("chmod busybox");
    let online_ctst = online_dir.join("app.ctst");
    write_busybox_ctst(&online_ctst, &format!("file://{}", rootfs.display()));
    let _ = run_checked(ctst().arg("build").arg(&online_ctst));
    let online_project = containust_common::constants::project_dir(&online_ctst);
    let digest = imported_digest(&online_project, "app");
    (online_project, digest)
}

#[cfg(target_os = "linux")]
fn stage_airgap_project(airgap_dir: &Path, online_project: &Path, digest: &str) -> PathBuf {
    std::fs::create_dir_all(airgap_dir).expect("mkdir airgap");
    let airgap_project = airgap_dir.join(".containust");
    copy_dir(
        &online_project.join("images"),
        &airgap_project.join("images"),
    );
    copy_dir(
        &online_project.join("layers"),
        &airgap_project.join("layers"),
    );
    let airgap_ctst = airgap_dir.join("app.ctst");
    write_busybox_ctst(&airgap_ctst, &format!("image://app@sha256:{digest}"));
    airgap_ctst
}

/// Full runtime pass of the exit gate: the air-gapped composition is
/// deployed with `--offline` and the container process actually runs.
///
/// Requires a static `busybox` binary on the host (for the fixture
/// rootfs) in addition to the usual privileged prerequisites.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires root privileges, user namespaces, cgroups v2, and busybox"]
fn gate_offline_run_starts_container_from_catalog() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let online_dir = workspace.path().join("online");
    let (online_project, digest) = import_busybox_online(&online_dir);
    let airgap_dir = workspace.path().join("airgap");
    let airgap_ctst = stage_airgap_project(&airgap_dir, &online_project, &digest);
    std::fs::remove_dir_all(&online_dir).expect("remove online project");

    let run = ctst()
        .arg("--offline")
        .arg("run")
        .arg("--detach")
        .arg(&airgap_ctst)
        .output()
        .expect("spawn ctst run");
    if !run.status.success() {
        let logs = std::fs::read_dir(airgap_dir.join(".containust/logs"))
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter_map(|e| std::fs::read_to_string(e.path()).ok())
            .collect::<Vec<_>>()
            .join("\n---\n");
        panic!(
            "ctst run failed\nstdout: {}\nstderr: {}\nlogs: {logs}",
            String::from_utf8_lossy(&run.stdout),
            String::from_utf8_lossy(&run.stderr)
        );
    }
    let ps = run_checked(ctst().current_dir(&airgap_dir).arg("ps").arg("--all"));
    let stdout = String::from_utf8_lossy(&ps.stdout);
    assert!(stdout.contains("app"), "stdout: {stdout}");
    assert!(stdout.contains("running"), "stdout: {stdout}");
    let _ = run_checked(ctst().current_dir(&airgap_dir).arg("stop").arg("--force"));
}
