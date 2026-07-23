//! Upgrade / rollback rehearsal for Sprint 8 (B8.3).
//!
//! Simulates a project workspace: migrate state across versions, survive an
//! interrupted write, and roll back state without losing logs or image catalog.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use containust_common::constants::STATE_SCHEMA_VERSION;
use containust_common::types::{ContainerId, ContainerState};
use containust_image::registry::ImageCatalog;
use containust_runtime::logs::{append_log, read_logs};
use containust_runtime::state::{StateEntry, StateFile, load_state, save_state};

fn project_layout(root: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let data = root.join(".containust");
    std::fs::create_dir_all(data.join("images")).expect("project dirs");
    let state = data.join("state.json");
    (state, data)
}

fn write_catalog(data_dir: &std::path::Path, name: &str) {
    let path = data_dir.join("images").join("catalog.json");
    let json = format!(
        r#"[{{
          "id": "img-{name}",
          "name": "{name}",
          "source": "file:///opt/{name}",
          "layers": [],
          "size_bytes": 1,
          "created_at": "2026-01-01T00:00:00Z",
          "digest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
          "tool_version": "0.8.0"
        }}]"#
    );
    std::fs::write(path, json).expect("write catalog");
}

fn sample_entry(name: &str) -> StateEntry {
    StateEntry {
        id: ContainerId::new(format!("id-{name}")),
        name: name.into(),
        state: ContainerState::Stopped,
        pid: None,
        image: "alpine:3.21".into(),
        command: Vec::new(),
        env: Vec::new(),
        memory_bytes: None,
        cpu_shares: None,
        readonly_rootfs: true,
        volumes: Vec::new(),
        rootfs_path: None,
        log_path: None,
        ports: Vec::new(),
        restart: containust_common::types::RestartPolicy::default(),
        healthcheck: None,
        health: None,
        restart_count: 0,
        created_at: "2026-01-01T00:00:00Z".into(),
    }
}

#[test]
fn upgrade_migrates_state_preserves_logs_and_catalog() {
    let root = tempfile::tempdir().expect("tempdir");
    let (state_path, data_dir) = project_layout(root.path());

    std::fs::write(
        &state_path,
        r#"{
          "schema_version": 1,
          "containers": [{
            "id": "id-web",
            "name": "web",
            "state": "Stopped",
            "image": "alpine:3.21",
            "created_at": "2026-01-01T00:00:00Z",
            "rootfs_path": null,
            "log_path": null,
            "pid": null
          }]
        }"#,
    )
    .expect("legacy state");
    append_log(&data_dir, "id-web", "boot ok").expect("log");
    write_catalog(&data_dir, "alpine");

    let migrated = load_state(&state_path).expect("migrate");
    assert_eq!(migrated.schema_version, STATE_SCHEMA_VERSION);
    assert_eq!(migrated.containers.len(), 1);
    save_state(&state_path, &migrated).expect("persist upgrade");
    std::fs::write(
        data_dir.join(".state.json.interrupted.tmp"),
        b"{\"schema_version\":2,",
    )
    .expect("partial temp");
    let after_interrupt = load_state(&state_path).expect("stable after interrupt");
    assert_eq!(after_interrupt.containers[0].name, "web");

    assert_eq!(read_logs(&data_dir, "id-web").expect("logs"), "boot ok\n");
    let images = ImageCatalog::open(&data_dir)
        .expect("catalog")
        .list()
        .expect("list");
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].name, "alpine");
}

#[test]
fn rollback_restores_state_backup_without_dropping_logs_or_catalog() {
    let root = tempfile::tempdir().expect("tempdir");
    let (state_path, data_dir) = project_layout(root.path());

    let mut current = StateFile::default();
    current.containers.push(sample_entry("web"));
    save_state(&state_path, &current).expect("current state");
    append_log(&data_dir, "id-web", "still here").expect("log");
    write_catalog(&data_dir, "keep-me");

    let backup = data_dir.join("state.json.bak");
    let _bytes = std::fs::copy(&state_path, &backup).expect("backup");

    let broken = StateFile::default();
    save_state(&state_path, &broken).expect("bad upgrade emptied state");

    let _bytes = std::fs::copy(&backup, &state_path).expect("restore");
    let restored = load_state(&state_path).expect("load restored");
    assert_eq!(restored.containers.len(), 1);
    assert_eq!(restored.containers[0].name, "web");
    assert_eq!(
        read_logs(&data_dir, "id-web").expect("logs"),
        "still here\n"
    );
    let images = ImageCatalog::open(&data_dir)
        .expect("catalog")
        .list()
        .expect("list");
    assert_eq!(images[0].name, "keep-me");
}
