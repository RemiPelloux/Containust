//! Test-only helpers shared by privileged fixtures.
//!
//! Only compiled for `cfg(test)` on Linux; nothing here ships in
//! release builds.

/// Runs `probe` in a forked, single-threaded child process and returns
/// whether it reported success.
///
/// `unshare(CLONE_NEWUSER)` fails with `EINVAL` in multithreaded
/// processes, and the libtest harness always runs tests on a spawned
/// thread — forking gives the probe a single-threaded process where
/// user-namespace syscalls behave as they do for a real container spawn.
///
/// The probe must not panic; it communicates failure by returning
/// `false`, which becomes the child's exit code.
// `pub` inside a private module keeps clippy::redundant_pub_crate happy;
// visibility is still crate-only because the module itself is private.
pub fn forked_probe_succeeds(probe: impl FnOnce() -> bool) -> bool {
    use nix::sys::wait::{WaitStatus, waitpid};
    use nix::unistd::{ForkResult, fork};

    // SAFETY: the child process only runs the probe closure and then
    // `_exit`s immediately; it never returns into the test harness, does
    // not unwind, and does not touch locks owned by other threads.
    match unsafe { fork() }.expect("fork test child") {
        ForkResult::Child => {
            let code = i32::from(!probe());
            // SAFETY: `_exit` is async-signal-safe and skips atexit
            // handlers, which is exactly what a forked test child needs.
            unsafe { nix::libc::_exit(code) }
        }
        ForkResult::Parent { child } => {
            matches!(waitpid(child, None), Ok(WaitStatus::Exited(_, 0)))
        }
    }
}
