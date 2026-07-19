//! Container log management.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use containust_common::error::{ContainustError, Result};

/// Returns the log file path for a container.
#[must_use]
pub fn log_path(data_dir: &Path, container_id: &str) -> PathBuf {
    data_dir.join("logs").join(format!("{container_id}.log"))
}

/// Reads container logs from disk.
///
/// Returns an empty string if the log file does not exist yet.
///
/// # Errors
///
/// Returns an error if the file exists but cannot be read.
pub fn read_logs(data_dir: &Path, container_id: &str) -> Result<String> {
    let path = log_path(data_dir, container_id);
    if !path.exists() {
        return Ok(String::new());
    }
    std::fs::read_to_string(&path).map_err(|e| ContainustError::Io { path, source: e })
}

/// Appends a log line for a container.
///
/// Creates the log directory and file if they do not exist.
///
/// # Errors
///
/// Returns an error if the directory or file cannot be created or written.
pub fn append_log(data_dir: &Path, container_id: &str, line: &str) -> Result<()> {
    let path = log_path(data_dir, container_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ContainustError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| ContainustError::Io {
            path: path.clone(),
            source: e,
        })?;
    writeln!(file, "{line}").map_err(|e| ContainustError::Io { path, source: e })?;
    Ok(())
}

/// Reads log bytes from an offset and returns the next offset.
///
/// This is intended for efficient tailing: callers do not need to reread
/// the complete log file on every poll.
///
/// # Errors
///
/// Returns an error if the log file cannot be opened, read, or positioned.
pub fn read_logs_from(data_dir: &Path, container_id: &str, offset: u64) -> Result<(String, u64)> {
    let path = log_path(data_dir, container_id);
    if !path.exists() {
        return Ok((String::new(), offset));
    }
    let mut file = std::fs::File::open(&path).map_err(|e| ContainustError::Io {
        path: path.clone(),
        source: e,
    })?;
    let length = file
        .metadata()
        .map_err(|e| ContainustError::Io {
            path: path.clone(),
            source: e,
        })?
        .len();
    let start = offset.min(length);
    let _ = file
        .seek(SeekFrom::Start(start))
        .map_err(|e| ContainustError::Io {
            path: path.clone(),
            source: e,
        })?;
    let mut bytes = Vec::new();
    let _ = file
        .read_to_end(&mut bytes)
        .map_err(|e| ContainustError::Io {
            path: path.clone(),
            source: e,
        })?;
    let next = start.saturating_add(bytes.len() as u64);
    Ok((String::from_utf8_lossy(&bytes).into_owned(), next))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_path_is_constructed_correctly() {
        let base = Path::new("/var/lib/containust");
        let p = log_path(base, "abc-123");
        assert_eq!(p, base.join("logs").join("abc-123.log"));
    }

    #[test]
    fn read_logs_missing_file_returns_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let content = read_logs(dir.path(), "nonexistent").expect("should succeed");
        assert!(content.is_empty());
    }

    #[test]
    fn append_and_read_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        append_log(dir.path(), "c1", "line one").expect("append 1");
        append_log(dir.path(), "c1", "line two").expect("append 2");

        let content = read_logs(dir.path(), "c1").expect("read");
        assert!(content.contains("line one"));
        assert!(content.contains("line two"));
    }

    #[test]
    fn append_creates_log_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let logs_dir = dir.path().join("logs");
        assert!(!logs_dir.exists());

        append_log(dir.path(), "c2", "first line").expect("append");
        assert!(logs_dir.exists());
    }

    #[test]
    fn separate_containers_have_separate_logs() {
        let dir = tempfile::tempdir().expect("tempdir");
        append_log(dir.path(), "a", "from a").expect("append a");
        append_log(dir.path(), "b", "from b").expect("append b");

        let a_logs = read_logs(dir.path(), "a").expect("read a");
        let b_logs = read_logs(dir.path(), "b").expect("read b");

        assert!(a_logs.contains("from a"));
        assert!(!a_logs.contains("from b"));
        assert!(b_logs.contains("from b"));
        assert!(!b_logs.contains("from a"));
    }

    #[test]
    fn read_logs_from_returns_incremental_content() {
        let dir = tempfile::tempdir().expect("tempdir");
        append_log(dir.path(), "c1", "first").expect("append first");
        let (first, offset) = read_logs_from(dir.path(), "c1", 0).expect("read first");
        append_log(dir.path(), "c1", "second").expect("append second");
        let (second, next) = read_logs_from(dir.path(), "c1", offset).expect("read second");

        assert!(first.contains("first"));
        assert!(!first.contains("second"));
        assert!(second.contains("second"));
        assert!(next > offset);
    }
}
