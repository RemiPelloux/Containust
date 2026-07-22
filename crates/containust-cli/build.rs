//! Inject reproducible build metadata for release binaries.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=CONTAINUST_GIT_SHA");
    println!("cargo:rerun-if-env-changed=CONTAINUST_BUILD_DATE");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");

    let sha = env_or("CONTAINUST_GIT_SHA").unwrap_or_else(git_sha);
    println!("cargo:rustc-env=CONTAINUST_GIT_SHA={sha}");

    let date = env_or("CONTAINUST_BUILD_DATE")
        .or_else(|| env_or("SOURCE_DATE_EPOCH").map(|epoch| format!("epoch:{epoch}")))
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=CONTAINUST_BUILD_DATE={date}");
}

fn env_or(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.is_empty())
}

fn git_sha() -> String {
    Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".into())
}
