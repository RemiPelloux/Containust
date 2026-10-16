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
    exec_in_container_timeout(container_id, pid, command, None)
}

/// Like [`exec_in_container`], but kills the command (and everything it
/// started in its process group) once `timeout` elapses.
///
/// # Errors
///
/// Returns an error if the command is empty, `nsenter` cannot be spawned,
/// or the timeout elapses.
#[cfg(target_os = "linux")]
pub fn exec_in_container_timeout(
    container_id: &ContainerId,
    pid: u32,
    command: &[String],
    timeout: Option<std::time::Duration>,
) -> Result<ExecOutput> {
    use std::os::unix::process::CommandExt;

    tracing::info!(id = %container_id, pid, cmd = ?command, "exec into container");
    if command.is_empty() {
        return Err(ContainustError::Config {
            message: "exec command is empty".into(),
        });
    }
    let mut child = std::process::Command::new("nsenter")
        .args(["--target", &pid.to_string(), "--mount", "--uts", "--ipc", "--net", "--pid", "--"])
        .args(command)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        // Own process group, so a timeout can kill whatever the command spawned.
        .process_group(0)
        .spawn()
        .map_err(|e| ContainustError::Io {
            path: "nsenter".into(),
            source: e,
        })?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let status = wait_or_kill(&mut child, timeout);
    // A probe's leftover background processes would keep the pipes open;
    // interactive `ctst exec` keeps them (users may background on purpose).
    if timeout.is_some() {
        kill_group(&child);
    }
    let output = ExecOutput {
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
        exit_code: status
            .as_ref()
            .ok()
            .and_then(std::process::ExitStatus::code)
            .unwrap_or(-1),
    };
    status
        .map(|_| output)
        .map_err(|elapsed| ContainustError::Config {
            message: format!(
                "command timed out after {}s: {command:?}",
                elapsed.as_secs()
            ),
        })
}

#[cfg(target_os = "linux")]
fn drain<R: std::io::Read + Send + 'static>(pipe: Option<R>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

/// Waits for `child`, killing its group once `timeout` elapses.
#[cfg(target_os = "linux")]
fn wait_or_kill(
    child: &mut std::process::Child,
    timeout: Option<std::time::Duration>,
) -> std::result::Result<std::process::ExitStatus, std::time::Duration> {
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(_) => return child.wait().map_err(|_| started.elapsed()),
        }
        if timeout.is_some_and(|limit| started.elapsed() >= limit) {
            kill_group(child);
            let _ = child.wait();
            return Err(started.elapsed());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[cfg(target_os = "linux")]
fn kill_group(child: &std::process::Child) {
    if let Ok(pid) = i32::try_from(child.id()) {
        let _ = nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(pid),
            nix::sys::signal::Signal::SIGKILL,
        );
    }
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

#[cfg(all(test, target_os = "linux"))]
#[allow(clippy::expect_used)]
mod tests {
    use std::os::unix::process::CommandExt;
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn timeout_kills_the_whole_process_group() {
        // A probe that backgrounds a child holding stdout open.
        let mut child = std::process::Command::new("sh")
            .args(["-c", "sleep 30 & echo started; sleep 30"])
            .stdout(std::process::Stdio::piped())
            .process_group(0)
            .spawn()
            .expect("spawn sh");
        let stdout = drain(child.stdout.take());
        let started = Instant::now();

        let result = wait_or_kill(&mut child, Some(Duration::from_millis(200)));

        assert!(result.is_err(), "must time out");
        assert!(started.elapsed() < Duration::from_secs(5));
        // The pipe closes only once the backgrounded sleep is dead too.
        assert_eq!(stdout.join().expect("reader").trim(), "started");
    }

    #[test]
    fn finished_command_returns_its_status() {
        let mut child = std::process::Command::new("sh")
            .args(["-c", "exit 3"])
            .process_group(0)
            .spawn()
            .expect("spawn sh");
        let status = wait_or_kill(&mut child, Some(Duration::from_secs(5))).expect("exits");
        assert_eq!(status.code(), Some(3));
    }
}
