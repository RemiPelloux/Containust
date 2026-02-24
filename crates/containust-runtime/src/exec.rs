//! Namespace joining for executing commands in running containers.

use containust_common::error::{ContainustError, Result};
use containust_common::types::ContainerId;

/// Output from an exec command.
#[derive(Debug, Clone)]
pub struct ExecOutput {
    /// Standard output from the command.
    pub stdout: String,
    /// Standard error from the command.
    pub stderr: String,
    /// Exit code returned by the command.
    pub exit_code: i32,
}

/// Joins the namespaces of a running container and executes a command.
///
/// Uses `nsenter` to enter the target container's mount, UTS, IPC,
/// network, and PID namespaces.
///
/// # Errors
///
/// Returns an error if the command is empty or `nsenter` invocation fails.
#[cfg(target_os = "linux")]
pub fn exec_in_container(
    container_id: &ContainerId,
    pid: u32,
    command: &[String],
) -> Result<ExecOutput> {
    tracing::info!(id = %container_id, pid, cmd = ?command, "exec into container");

    if command.is_empty() {
        return Err(ContainustError::Config {
            message: "exec command is empty".into(),
        });
    }

    let output = std::process::Command::new("nsenter")
        .args([
            "--target",
            &pid.to_string(),
            "--mount",
            "--uts",
            "--ipc",
            "--net",
            "--pid",
            "--",
        ])
        .args(command)
        .output()
        .map_err(|e| ContainustError::Io {
            path: "nsenter".into(),
            source: e,
        })?;

    Ok(ExecOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}

/// Joins the namespaces of a running container and executes a command.
///
/// On non-Linux platforms, returns an error because namespace
/// operations require the Linux kernel.
///
/// # Errors
///
/// Always returns an error on non-Linux platforms.
#[cfg(not(target_os = "linux"))]
pub fn exec_in_container(
    _container_id: &ContainerId,
    _pid: u32,
    _command: &[String],
) -> Result<ExecOutput> {
    Err(ContainustError::Config {
        message: "exec requires Linux (use VM backend on macOS/Windows)".into(),
    })
}
