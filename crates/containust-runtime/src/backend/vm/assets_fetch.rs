//! Resumable, locked downloads for pinned VM boot assets.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use containust_common::error::{ContainustError, Result};
use containust_common::types::Sha256Hash;
use fs2::FileExt;

/// Exclusive lock held for the duration of a cache update.
pub struct CacheLock {
    file: std::fs::File,
    path: PathBuf,
}

impl CacheLock {
    /// Acquires an exclusive lock under `cache_dir/.assets.lock`.
    pub fn acquire(cache_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(cache_dir).map_err(|source| ContainustError::Io {
            path: cache_dir.to_path_buf(),
            source,
        })?;
        let path = cache_dir.join(".assets.lock");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|source| ContainustError::Io {
                path: path.clone(),
                source,
            })?;
        FileExt::lock_exclusive(&file).map_err(|source| ContainustError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(Self { file, path })
    }
}

impl Drop for CacheLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
        let _ = &self.path;
    }
}

/// Downloads `url` into `dest`, resuming from a `.partial` file when possible.
///
/// Verifies the final blob against `expected` and atomically renames it into
/// place. On digest mismatch the partial file is deleted.
pub fn download_resumable(url: &str, dest: &Path, expected: &Sha256Hash) -> Result<()> {
    let staging = partial_path(dest);
    let existing = staging_len(&staging);
    if existing > 0 {
        eprintln!("  Resuming download from {existing} bytes...");
    }
    fetch_into_staging(url, &staging, existing)?;
    if let Err(error) = containust_image::hash::validate_hash(&staging, expected) {
        let _ = std::fs::remove_file(&staging);
        return Err(error);
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ContainustError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::rename(&staging, dest).map_err(|source| ContainustError::Io {
        path: dest.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn partial_path(dest: &Path) -> PathBuf {
    let mut staging = dest.as_os_str().to_os_string();
    staging.push(".partial");
    PathBuf::from(staging)
}

fn staging_len(staging: &Path) -> u64 {
    std::fs::metadata(staging).map_or(0, |meta| meta.len())
}

fn fetch_into_staging(url: &str, staging: &Path, existing: u64) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| ContainustError::Network {
            url: url.to_string(),
            message: format!("failed to construct HTTP client: {e}"),
        })?;
    let mut request = client.get(url);
    if existing > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={existing}-"));
    }
    let response = request.send().map_err(|e| ContainustError::Network {
        url: url.to_string(),
        message: format!("failed to download: {e}"),
    })?;
    let status = response.status();
    if status.as_u16() == 416 && existing > 0 {
        // Server says the range is unsatisfiable — partial is already complete.
        return Ok(());
    }
    if !status.is_success() && status.as_u16() != 206 {
        return Err(ContainustError::Network {
            url: url.to_string(),
            message: format!("HTTP {status} downloading asset"),
        });
    }
    let append = status.as_u16() == 206 && existing > 0;
    stream_body(response, staging, append, url)
}

fn stream_body(
    response: reqwest::blocking::Response,
    staging: &Path,
    append: bool,
    url: &str,
) -> Result<()> {
    let io_error = |source| ContainustError::Io {
        path: staging.to_path_buf(),
        source,
    };
    let mut file = if append {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(staging)
            .map_err(io_error)?
    } else {
        std::fs::File::create(staging).map_err(io_error)?
    };
    let mut reader = response;
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut written = if append {
        file.seek(SeekFrom::End(0)).map_err(io_error)?
    } else {
        0
    };
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|e| ContainustError::Network {
                url: url.to_string(),
                message: format!("stream interrupted after {written} bytes: {e}"),
            })?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read]).map_err(io_error)?;
        written += read as u64;
    }
    file.sync_all().map_err(io_error)?;
    #[allow(clippy::cast_precision_loss)]
    {
        let mb = written as f64 / 1_048_576.0;
        eprintln!("  Downloaded {mb:.1} MB");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};

    fn read_http_request(stream: &mut TcpStream) -> String {
        let mut request = String::new();
        let mut reader = std::io::BufReader::new(stream);
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
                break;
            }
            request.push_str(&line);
        }
        request
    }

    fn parse_range_start(request: &str) -> Option<usize> {
        request.lines().find_map(|line| {
            line.trim()
                .strip_prefix("Range: bytes=")
                .and_then(|r| r.trim().strip_suffix('-'))
                .and_then(|n| n.parse::<usize>().ok())
        })
    }

    fn serve_body(
        body: &'static [u8],
        support_range: bool,
    ) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let handle = std::thread::spawn(move || {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            let mut stream = stream;
            let request = read_http_request(&mut stream);
            let range = parse_range_start(&request);
            let (status, payload) = match (support_range, range) {
                (true, Some(start)) if start < body.len() => {
                    ("HTTP/1.1 206 Partial Content", &body[start..])
                }
                (true, Some(start)) if start >= body.len() => {
                    let _ = stream.write_all(
                        b"HTTP/1.1 416 Range Not Satisfiable\r\nConnection: close\r\ncontent-length: 0\r\n\r\n",
                    );
                    return;
                }
                _ => ("HTTP/1.1 200 OK", body),
            };
            let header = format!(
                "{status}\r\nConnection: close\r\ncontent-length: {}\r\n\r\n",
                payload.len()
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(payload);
        });
        (format!("http://127.0.0.1:{port}/asset.bin"), handle)
    }

    #[test]
    fn download_resumable_completes_from_partial() {
        const BODY: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
        let digest = {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(BODY))
        };
        let expected = Sha256Hash::from_hex(&digest).expect("hex");
        let (url, handle) = serve_body(BODY, true);
        let dir = tempfile::tempdir().expect("tempdir");
        let dest = dir.path().join("asset.bin");
        let staging = partial_path(&dest);
        std::fs::write(&staging, &BODY[..10]).expect("seed partial");

        download_resumable(&url, &dest, &expected).expect("resume");

        assert_eq!(std::fs::read(&dest).expect("read"), BODY);
        assert!(!staging.exists());
        let _ = handle.join();
    }

    #[test]
    fn cache_lock_serializes_two_acquisitions() {
        let dir = tempfile::tempdir().expect("tempdir");
        let order = Arc::new(Mutex::new(Vec::new()));
        let ready = Arc::new(std::sync::Barrier::new(2));
        let order_a = Arc::clone(&order);
        let order_b = Arc::clone(&order);
        let ready_a = Arc::clone(&ready);
        let ready_b = Arc::clone(&ready);
        let path = dir.path().to_path_buf();
        let path_b = path.clone();
        let t1 = std::thread::spawn(move || {
            let lock = CacheLock::acquire(&path).expect("lock a");
            order_a.lock().expect("m").push(1);
            ready_a.wait();
            std::thread::sleep(std::time::Duration::from_millis(50));
            order_a.lock().expect("m").push(2);
            drop(lock);
        });
        let t2 = std::thread::spawn(move || {
            ready_b.wait();
            let lock = CacheLock::acquire(&path_b).expect("lock b");
            order_b.lock().expect("m").push(3);
            drop(lock);
        });
        t1.join().expect("t1");
        t2.join().expect("t2");
        let seen = order.lock().expect("m").clone();
        // Holder logs 1..2 under the lock; waiter must observe 3 only after release.
        assert_eq!(seen, vec![1, 2, 3]);
    }
}
