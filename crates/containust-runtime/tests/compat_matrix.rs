//! Compatibility matrix checks for Sprint 8 (B8.2).

#![allow(clippy::expect_used, clippy::unwrap_used)]

use containust_common::codes;
use containust_common::constants::STATE_SCHEMA_VERSION;
use containust_compose::parser::parse_ctst;
use containust_compose::resolver::resolve_connections;
use containust_runtime::state::{StateFile, load_state, save_state};

#[test]
fn legacy_state_schema_one_migrates_to_current() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("state.json");
    std::fs::write(
        &path,
        r#"{
          "schema_version": 1,
          "containers": []
        }"#,
    )
    .expect("write legacy state");

    let loaded = load_state(&path).expect("load migrates");
    assert_eq!(loaded.schema_version, STATE_SCHEMA_VERSION);
    assert!(loaded.containers.is_empty());

    save_state(&path, &loaded).expect("save");
    let roundtrip = load_state(&path).expect("reload");
    assert_eq!(roundtrip.schema_version, STATE_SCHEMA_VERSION);
}

#[test]
fn future_state_schema_is_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("state.json");
    let future = STATE_SCHEMA_VERSION + 1;
    std::fs::write(
        &path,
        format!(r#"{{"schema_version":{future},"containers":[]}}"#),
    )
    .expect("write future state");

    let err = load_state(&path).expect_err("reject future schema");
    assert!(err.to_string().contains("schema"));
}

#[test]
fn ctst_parse_and_resolve_stable_surface() {
    let source = r#"
COMPONENT web {
  image = "alpine:3.21"
  port = 8080
}
COMPONENT db {
  image = "alpine:3.21"
}
CONNECT web -> db
"#;
    let file = parse_ctst(source).expect("parse");
    assert_eq!(file.components.len(), 2);
    assert_eq!(file.connections.len(), 1);
    let resolved = resolve_connections(&file).expect("resolve");
    assert_eq!(resolved.len(), 2);
}

#[test]
fn error_codes_cover_offline_and_not_found() {
    let offline = codes::classify_message("offline mode rejects remote source: https://example.com");
    assert_eq!(offline.code, "I004");
    let missing = codes::classify_message("container not found: abc");
    assert_eq!(missing.code, "R001");
    assert!(missing.exit_code > 0);
}

#[test]
fn empty_state_file_defaults_to_current_schema() {
    let index = StateFile::default();
    assert_eq!(index.schema_version, STATE_SCHEMA_VERSION);
}
