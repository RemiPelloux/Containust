//! Host process liveness and termination helpers for the VM backend.

use std::time::Duration;

const GRACEFUL_WAIT: Duration = Duration::from_secs(5);
const GRACEFUL_POLL: Duration = Duration::from_millis(100);

/// Returns whether `pid` still exists on the host.
#[must_use]
pub fn process_is_alive(pid: u32) -> bool {
    process_is_alive_impl(pid)
}

/// Terminates `pid`. When `force` is false, prefers SIGTERM then SIGKILL.
pub fn terminate_pid(pid: u32, force: bool) {
    terminate_pid_impl(pid, force);
}

/// Waits until `pid` exits or `budget` elapses. Returns true if dead.
pub fn wait_until_dead(pid: u32, budget: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < budget {
        if !process_is_alive(pid) {
            return true;
        }
        std::thread::sleep(GRACEFUL_POLL);
    }
    !process_is_alive(pid)
}

#[cfg(unix)]
fn process_is_alive_impl(pid: u32) -> bool {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;

    let Ok(raw) = i32::try_from(pid) else {
        return false;
    };
    kill(Pid::from_raw(raw), None).is_ok()
}

#[cfg(unix)]
fn terminate_pid_impl(pid: u32, force: bool) {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;

    let Ok(raw) = i32::try_from(pid) else {
        return;
    };
    let nix_pid = Pid::from_raw(raw);
    if force {
        let _ = signal::kill(nix_pid, Signal::SIGKILL);
        return;
    }
    if signal::kill(nix_pid, Signal::SIGTERM).is_err() {
        return;
    }
    tracing::info!(pid, "sent SIGTERM to QEMU");
    if !wait_until_dead(pid, GRACEFUL_WAIT) {
        let _ = signal::kill(nix_pid, Signal::SIGKILL);
        tracing::info!(pid, "escalated to SIGKILL");
    }
}

#[cfg(windows)]
fn process_is_alive_impl(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, WaitForSingleObject,
    };

    // SAFETY: Query-only process handle; CloseHandle always paired.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let status = WaitForSingleObject(handle, 0);
        let _ = CloseHandle(handle);
        status != WAIT_OBJECT_0
    }
}

#[cfg(windows)]
fn terminate_pid_impl(pid: u32, _force: bool) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};

    // SAFETY: TerminateProcess on a handle opened with PROCESS_TERMINATE only.
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if handle.is_null() {
            return;
        }
        let _ = TerminateProcess(handle, 1);
        let _ = CloseHandle(handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_is_alive_current_process() {
        assert!(process_is_alive(std::process::id()));
    }
}
