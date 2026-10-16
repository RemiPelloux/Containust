//! `ctst plan` declarative diff contract (Sprint 12).

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::process::Command;

fn ctst() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ctst"))
}

const STACK: &str = r#"COMPONENT db {
    image = "file:///opt/images/db"
    port = 5432
}

COMPONENT api {
    image = "file:///opt/images/api"
    port = 8080
}

CONNECT api -> db
"#;

#[test]
fn plan_json_lists_creates_in_dependency_order_without_touching_disk() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("stack.ctst");
    std::fs::write(&file, STACK).expect("write");

    let output = ctst()
        .arg("plan")
        .arg("--json")
        .arg(&file)
        .output()
        .expect("run ctst");
    assert!(output.status.success(), "{output:?}");
    let plan: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json plan");
    let changes = plan["changes"].as_array().expect("changes");
    let names: Vec<_> = changes
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    let actions: Vec<_> = changes
        .iter()
        .map(|c| c["action"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["db", "api"]);
    assert_eq!(actions, ["create", "create"]);
    assert!(
        !dir.path().join(".containust").join("state").exists(),
        "plan must be read-only"
    );
}

#[test]
fn plan_detailed_exitcode_signals_pending_changes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("stack.ctst");
    std::fs::write(&file, STACK).expect("write");

    let output = ctst()
        .arg("plan")
        .arg("--detailed-exitcode")
        .arg(&file)
        .output()
        .expect("run ctst");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("2 to create"), "stdout: {stdout}");
    assert!(
        stdout.contains("file:///opt/images/api"),
        "stdout: {stdout}"
    );
}

#[test]
fn detailed_exitcode_never_reports_an_error_as_drift() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("broken.ctst");
    // A dependency cycle is classified as an invalid composition (exit 2
    // without the flag), which must not read as "changes pending".
    std::fs::write(
        &file,
        "COMPONENT a {\n image = \"file:///a\"\n}\nCOMPONENT b {\n image = \"file:///b\"\n}\n\
         CONNECT a -> b\nCONNECT b -> a\n",
    )
    .expect("write");

    let output = ctst()
        .arg("plan")
        .arg("--detailed-exitcode")
        .arg(&file)
        .output()
        .expect("run ctst");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
}
