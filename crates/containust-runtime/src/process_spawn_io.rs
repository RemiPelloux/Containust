//! Pipe, stdio, argv, and env helpers for the user/PID spawn path.

#![cfg(target_os = "linux")]

use std::ffi::CString;
use std::io::Read;
use std::os::fd::{AsFd, FromRawFd, IntoRawFd};
use std::path::Path;

use containust_common::error::{ContainustError, Result};
use nix::unistd::{dup2_stderr, dup2_stdin, dup2_stdout, pipe, read, write};

use crate::process::ProcessConfig;

pub fn open_log_fds(config: &ProcessConfig) -> Result<Option<(std::fs::File, std::fs::File)>> {
    let Some(log_path) = &config.log_path else {
        return Ok(None);
    };
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ContainustError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let stdout = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|source| ContainustError::Io {
            path: log_path.clone(),
            source,
        })?;
    let stderr = stdout.try_clone().map_err(|source| ContainustError::Io {
        path: log_path.clone(),
        source,
    })?;
    Ok(Some((stdout, stderr)))
}

pub fn redirect_stdio(log_fds: Option<(std::fs::File, std::fs::File)>) -> std::io::Result<()> {
    let devnull = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")?;
    dup2_stdin(&devnull).map_err(std::io::Error::from)?;
    if let Some((stdout, stderr)) = log_fds {
        dup2_stdout(&stdout).map_err(std::io::Error::from)?;
        dup2_stderr(&stderr).map_err(std::io::Error::from)?;
    }
    Ok(())
}

pub fn c_strings(args: &[String]) -> Result<Vec<CString>> {
    args.iter()
        .map(|s| {
            CString::new(s.as_str()).map_err(|_| ContainustError::Config {
                message: format!("command argument contains interior NUL: {s:?}"),
            })
        })
        .collect()
}

pub fn build_envp(config: &ProcessConfig) -> Result<Vec<CString>> {
    let mut env = vec![
        "PATH=/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin".to_string(),
        "HOME=/root".to_string(),
        "TERM=xterm".to_string(),
    ];
    for (key, value) in &config.env {
        env.push(format!("{key}={value}"));
    }
    c_strings(&env)
}

pub fn pipe_pair() -> Result<(std::fs::File, std::fs::File)> {
    let (read_fd, write_fd) = pipe().map_err(|e| ContainustError::Config {
        message: format!("pipe failed: {e}"),
    })?;
    // SAFETY: uniquely owned fresh pipe ends.
    let read = unsafe { std::fs::File::from_raw_fd(read_fd.into_raw_fd()) };
    let write = unsafe { std::fs::File::from_raw_fd(write_fd.into_raw_fd()) };
    Ok((read, write))
}

pub fn write_all_file(file: &impl AsFd, buf: &[u8]) -> std::io::Result<()> {
    let mut offset = 0;
    while offset < buf.len() {
        match write(file, &buf[offset..]) {
            Ok(0) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "write returned 0",
                ));
            }
            Ok(n) => offset += n,
            Err(nix::errno::Errno::EINTR) => {}
            Err(err) => return Err(std::io::Error::from(err)),
        }
    }
    Ok(())
}

pub fn read_one_file(file: &impl AsFd) -> std::io::Result<u8> {
    let mut buf = [0_u8; 1];
    loop {
        match read(file, &mut buf) {
            Ok(0) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "sync pipe closed",
                ));
            }
            Ok(_) => return Ok(buf[0]),
            Err(nix::errno::Errno::EINTR) => {}
            Err(err) => return Err(std::io::Error::from(err)),
        }
    }
}

pub fn read_exact_file(file: &mut std::fs::File, buf: &mut [u8]) -> Result<()> {
    file.read_exact(buf).map_err(|source| ContainustError::Io {
        path: Path::new("spawn-sync-pipe").to_path_buf(),
        source,
    })
}

/// Drops a file descriptor by consuming the `File` (preferred over raw close).
pub fn drop_fd(file: std::fs::File) {
    drop(file);
}
