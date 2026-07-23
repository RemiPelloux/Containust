//! Linux spawn orchestration for user and PID namespaces.
//!
//! Uses a manual `fork` + pipe handshake (not `Command::spawn`) so the
//! intermediate process can `_exit` after a PID-namespace double-fork
//! without being misreported as an exec failure.

#![cfg(target_os = "linux")]

use std::ffi::CString;
use std::io::Write;
use std::path::Path;

use containust_common::error::{ContainustError, Result};
use containust_core::namespace::NamespaceConfig;
use nix::unistd::{ForkResult, execvp, fork};

use crate::process::ProcessConfig;
use crate::process_spawn_io::{
    build_envp, c_strings, drop_fd, open_log_fds, pipe_pair, read_exact_file, read_one_file,
    redirect_stdio, write_all_file,
};

const MSG_NEED_MAPS: u8 = 1;
const MSG_MAPS_DONE: u8 = 2;
const MSG_INIT_PID: u8 = 3;

/// Spawns with user and/or PID namespace support.
pub fn spawn_with_user_pid(config: &ProcessConfig) -> Result<u32> {
    validate_spawn_inputs(config)?;
    let (parent_rx, child_tx) = pipe_pair()?;
    let (child_rx, parent_tx) = pipe_pair()?;
    let log_fds = open_log_fds(config)?;
    let argv = c_strings(&config.command)?;
    let envp = build_envp(config)?;
    let child_cfg = ChildConfig {
        rootfs: config.rootfs.clone(),
        volumes: config.volumes.clone(),
        readonly_rootfs: config.readonly_rootfs,
        namespaces: config.namespaces.clone(),
    };

    // SAFETY: child never returns into the parent Rust stack.
    let fork_result = unsafe { fork() }.map_err(|e| ContainustError::Config {
        message: format!("fork failed: {e}"),
    })?;

    match fork_result {
        ForkResult::Parent { child } => {
            drop_fd(child_tx);
            drop_fd(child_rx);
            let spawn_pid = u32::try_from(child.as_raw()).unwrap_or(u32::MAX);
            let init_pid = parent_handshake(parent_rx, parent_tx, spawn_pid, &config.namespaces)?;
            if config.namespaces.pid {
                let _ = nix::sys::wait::waitpid(child, None);
            }
            tracing::info!(pid = init_pid, "container process spawned (user/pid path)");
            Ok(init_pid)
        }
        ForkResult::Child => {
            drop_fd(parent_rx);
            drop_fd(parent_tx);
            let pipes = ChildPipes {
                tx: child_tx,
                rx: child_rx,
            };
            let exec = ExecArgs {
                argv: &argv,
                envp: &envp,
            };
            if let Err(err) = child_main(&child_cfg, pipes, log_fds, exec) {
                let _ = writeln!(std::io::stderr(), "containust spawn child failed: {err}");
                // SAFETY: child must not unwind into the parent address space.
                unsafe { libc::_exit(1) };
            }
            // SAFETY: exec only returns on failure.
            unsafe { libc::_exit(1) };
        }
    }
}

struct ChildConfig {
    rootfs: std::path::PathBuf,
    volumes: Vec<String>,
    readonly_rootfs: bool,
    namespaces: NamespaceConfig,
}

struct ChildPipes {
    tx: std::fs::File,
    rx: std::fs::File,
}

struct ExecArgs<'a> {
    argv: &'a [CString],
    envp: &'a [CString],
}

fn validate_spawn_inputs(config: &ProcessConfig) -> Result<()> {
    if config.command.is_empty() {
        return Err(ContainustError::Config {
            message: "container command is empty".into(),
        });
    }
    if !config.rootfs.exists() {
        return Err(ContainustError::Config {
            message: format!(
                "rootfs directory does not exist: {}",
                config.rootfs.display()
            ),
        });
    }
    Ok(())
}

fn child_main(
    cfg: &ChildConfig,
    pipes: ChildPipes,
    log_fds: Option<(std::fs::File, std::fs::File)>,
    exec: ExecArgs<'_>,
) -> std::io::Result<()> {
    redirect_stdio(log_fds)?;
    if cfg.namespaces.user {
        containust_core::namespace::user::create_user_namespace()
            .map_err(|e| std::io::Error::other(format!("user namespace failed: {e}")))?;
        write_all_file(&pipes.tx, &[MSG_NEED_MAPS])?;
        if read_one_file(&pipes.rx)? != MSG_MAPS_DONE {
            return Err(std::io::Error::other("parent did not acknowledge uid maps"));
        }
    }
    unshare_remaining(&cfg.namespaces)?;
    enter_pid_and_exec(cfg, pipes, exec)
}

fn enter_pid_and_exec(
    cfg: &ChildConfig,
    pipes: ChildPipes,
    exec: ExecArgs<'_>,
) -> std::io::Result<()> {
    if !cfg.namespaces.pid {
        drop_fd(pipes.tx);
        drop_fd(pipes.rx);
        crate::process::configure_child_isolation_after_ns(
            &cfg.rootfs,
            &cfg.volumes,
            cfg.readonly_rootfs,
        )?;
        return exec_container(exec);
    }
    // SAFETY: child is still single-threaded.
    match unsafe { fork() } {
        Ok(ForkResult::Parent { .. }) => {
            // SAFETY: intermediate exits; grandchild is PID 1 in the ns.
            unsafe { libc::_exit(0) }
        }
        Ok(ForkResult::Child) => {
            // getpid() is 1 inside the new PID ns — read the host PID from
            // the still-mounted host /proc before pivot_root.
            let host_pid = host_pid_from_proc_self()?;
            write_all_file(&pipes.tx, &[MSG_INIT_PID])?;
            write_all_file(&pipes.tx, &host_pid.to_le_bytes())?;
            drop_fd(pipes.tx);
            drop_fd(pipes.rx);
            crate::process::configure_child_isolation_after_ns(
                &cfg.rootfs,
                &cfg.volumes,
                cfg.readonly_rootfs,
            )?;
            exec_container(exec)
        }
        Err(err) => Err(std::io::Error::other(format!("pid-ns fork failed: {err}"))),
    }
}

/// Host PID via `/proc/self` while the host procfs is still mounted.
fn host_pid_from_proc_self() -> std::io::Result<u32> {
    let link = std::fs::read_link("/proc/self")?;
    let text = link.to_string_lossy();
    text.parse::<u32>()
        .map_err(|e| std::io::Error::other(format!("parse /proc/self ({text}): {e}")))
}

fn exec_container(exec: ExecArgs<'_>) -> std::io::Result<()> {
    apply_env(exec.envp);
    let refs: Vec<&std::ffi::CStr> = exec.argv.iter().map(CString::as_c_str).collect();
    match execvp(refs[0], &refs) {
        Ok(infallible) => match infallible {},
        Err(err) => Err(std::io::Error::other(format!("execvp failed: {err}"))),
    }
}

fn apply_env(envp: &[CString]) {
    for (key, _) in std::env::vars_os() {
        let Ok(key) = CString::new(key.to_string_lossy().as_bytes()) else {
            continue;
        };
        // SAFETY: key is a NUL-terminated CString from the process environment.
        unsafe {
            libc::unsetenv(key.as_ptr());
        }
    }
    for var in envp {
        let bytes = var.as_bytes();
        let Some(eq) = bytes.iter().position(|b| *b == b'=') else {
            continue;
        };
        let Ok(k) = CString::new(&bytes[..eq]) else {
            continue;
        };
        let Ok(v) = CString::new(&bytes[eq + 1..]) else {
            continue;
        };
        // SAFETY: k/v are NUL-terminated CStrings built above.
        unsafe {
            libc::setenv(k.as_ptr(), v.as_ptr(), 1);
        }
    }
}

fn parent_handshake(
    mut parent_rx: std::fs::File,
    mut parent_tx: std::fs::File,
    spawn_pid: u32,
    namespaces: &NamespaceConfig,
) -> Result<u32> {
    let mut tag = [0_u8; 1];
    read_exact_file(&mut parent_rx, &mut tag)?;
    if namespaces.user {
        if tag[0] != MSG_NEED_MAPS {
            return Err(handshake_err("NEED_MAPS", tag[0]));
        }
        let host_uid = nix::unistd::geteuid().as_raw();
        containust_core::namespace::user::write_uid_gid_map(spawn_pid, 0, host_uid, 1)?;
        parent_tx
            .write_all(&[MSG_MAPS_DONE])
            .map_err(|source| ContainustError::Io {
                path: Path::new("spawn-sync-pipe").to_path_buf(),
                source,
            })?;
        if !namespaces.pid {
            return Ok(spawn_pid);
        }
        read_exact_file(&mut parent_rx, &mut tag)?;
    }
    if namespaces.pid {
        if tag[0] != MSG_INIT_PID {
            return Err(handshake_err("INIT_PID", tag[0]));
        }
        let mut pid_buf = [0_u8; 4];
        read_exact_file(&mut parent_rx, &mut pid_buf)?;
        return Ok(u32::from_le_bytes(pid_buf));
    }
    Ok(spawn_pid)
}

fn unshare_remaining(namespaces: &NamespaceConfig) -> std::io::Result<()> {
    use nix::sched::{CloneFlags, unshare};

    let mut flags = CloneFlags::empty();
    if namespaces.mount {
        flags |= CloneFlags::CLONE_NEWNS;
    }
    if namespaces.pid {
        flags |= CloneFlags::CLONE_NEWPID;
    }
    if namespaces.network {
        flags |= CloneFlags::CLONE_NEWNET;
    }
    if namespaces.ipc {
        flags |= CloneFlags::CLONE_NEWIPC;
    }
    if namespaces.uts {
        flags |= CloneFlags::CLONE_NEWUTS;
    }
    if flags.is_empty() {
        return Ok(());
    }
    unshare(flags).map_err(|e| std::io::Error::other(format!("unshare failed: {e}")))
}

fn handshake_err(expected: &str, got: u8) -> ContainustError {
    ContainustError::Config {
        message: format!("spawn handshake: expected {expected}, got {got}"),
    }
}
