//! Explicit opt-in remote image downloader.
//!
//! Remote sources are never fetched implicitly: the caller must supply
//! a [`FetchPolicy`], the reference must pin a SHA-256 digest, and
//! offline mode rejects the request before any connection is opened.

use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use containust_common::error::{ContainustError, Result};
use containust_common::types::Sha256Hash;

use crate::reference::ImageReference;

/// Network policy for a remote fetch.
#[derive(Debug, Clone)]
pub struct FetchPolicy {
    /// Total request timeout.
    pub timeout: Duration,
    /// Maximum number of HTTP redirects to follow.
    pub max_redirects: usize,
    /// Maximum accepted payload size in bytes.
    pub max_bytes: u64,
    /// Number of retries after the first failed attempt.
    pub retries: u32,
    /// When true, reject the fetch before opening any connection.
    pub offline: bool,
}

impl Default for FetchPolicy {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(60),
            max_redirects: 5,
            max_bytes: 2 * 1024 * 1024 * 1024,
            retries: 2,
            offline: false,
        }
    }
}

/// Downloads a remote image archive to `destination` and verifies it.
///
/// The reference must be remote and must pin an expected SHA-256
/// digest. On digest mismatch the downloaded file is deleted.
///
/// # Errors
///
/// Returns an error when offline mode is enabled, the digest is
/// missing, the size limit is exceeded, all retries fail, or the
/// downloaded content does not match the pinned digest.
pub fn fetch_remote(
    reference: &ImageReference,
    policy: &FetchPolicy,
    destination: &Path,
) -> Result<Sha256Hash> {
    let url = reference.canonical_uri();
    if policy.offline {
        return Err(ContainustError::Network {
            url,
            message: "offline mode blocks remote image fetch; import the image on a \
                      connected machine and copy the layer store"
                .into(),
        });
    }
    let Some(expected) = reference.digest() else {
        return Err(ContainustError::Network {
            url,
            message: "remote sources require a pinned digest \
                      (append @sha256:<hex> to the image URI)"
                .into(),
        });
    };

    download_with_retries(&url, policy, destination)?;
    verify_download(destination, expected)?;
    tracing::info!(url = %url, digest = %expected, "remote image fetched and verified");
    Ok(expected.clone())
}

fn download_with_retries(url: &str, policy: &FetchPolicy, destination: &Path) -> Result<()> {
    let client = build_client(policy).map_err(|error| ContainustError::Network {
        url: url.to_string(),
        message: format!("failed to construct HTTP client: {error}"),
    })?;
    let mut last_error = String::from("no attempt was made");
    for attempt in 0..=policy.retries {
        match download_once(&client, url, policy, destination) {
            Ok(()) => return Ok(()),
            Err(error) => {
                tracing::warn!(url, attempt, %error, "remote fetch attempt failed");
                last_error = error.to_string();
            }
        }
    }
    Err(ContainustError::Network {
        url: url.to_string(),
        message: format!(
            "download failed after {} attempt(s): {last_error}",
            policy.retries + 1
        ),
    })
}

fn build_client(policy: &FetchPolicy) -> reqwest::Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(policy.timeout)
        .redirect(reqwest::redirect::Policy::limited(policy.max_redirects))
        .build()
}

fn download_once(
    client: &reqwest::blocking::Client,
    url: &str,
    policy: &FetchPolicy,
    destination: &Path,
) -> Result<()> {
    let network_error = |message: String| ContainustError::Network {
        url: url.to_string(),
        message,
    };
    let response = client
        .get(url)
        .send()
        .map_err(|error| network_error(format!("request failed: {error}")))?;
    let status = response.status();
    if !status.is_success() {
        return Err(network_error(format!("server returned status {status}")));
    }
    if response
        .content_length()
        .is_some_and(|n| n > policy.max_bytes)
    {
        return Err(network_error(format!(
            "declared payload exceeds the {} byte limit",
            policy.max_bytes
        )));
    }
    copy_capped(response, destination, policy.max_bytes, url)
}

fn copy_capped(
    response: reqwest::blocking::Response,
    destination: &Path,
    max_bytes: u64,
    url: &str,
) -> Result<()> {
    let mut file = std::fs::File::create(destination).map_err(|source| ContainustError::Io {
        path: destination.to_path_buf(),
        source,
    })?;
    let mut reader = response.take(max_bytes.saturating_add(1));
    let mut written: u64 = 0;
    let mut buffer = vec![0_u8; 64 * 1024];
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
        written += read as u64;
        if written > max_bytes {
            let _ = std::fs::remove_file(destination);
            return Err(ContainustError::Network {
                url: url.to_string(),
                message: format!("payload exceeds the {max_bytes} byte limit"),
            });
        }
        file.write_all(&buffer[..read])
            .map_err(|source| ContainustError::Io {
                path: destination.to_path_buf(),
                source,
            })?;
    }
    file.sync_all().map_err(|source| ContainustError::Io {
        path: destination.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn verify_download(destination: &Path, expected: &Sha256Hash) -> Result<()> {
    if let Err(error) = crate::hash::validate_hash(destination, expected) {
        let _ = std::fs::remove_file(destination);
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    const BODY: &[u8] = b"remote layer bytes";
    const BODY_DIGEST: &str = "9a373b7f523aa05bf3624c1bf41ee0659ca7a0f24e71a94b1f1d38dd58245733";

    /// Handles one HTTP request; returns true when the client asked
    /// the server to stop.
    fn handle_connection(
        mut stream: std::net::TcpStream,
        body: &[u8],
        remaining_failures: &AtomicU32,
    ) -> bool {
        let mut reader = std::io::BufReader::new(&mut stream);
        let mut line = String::new();
        let _ = reader.read_line(&mut line);
        loop {
            let mut header = String::new();
            if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
                break;
            }
        }
        let should_fail = remaining_failures
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .is_ok();
        let response = if should_fail {
            b"HTTP/1.1 500 Internal Server Error\r\ncontent-length: 0\r\n\r\n".to_vec()
        } else {
            let mut ok =
                format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n", body.len()).into_bytes();
            ok.extend_from_slice(body);
            ok
        };
        let _ = stream.write_all(&response);
        line.contains("/stop")
    }

    /// Accepts connections until a `/stop` request arrives.
    fn accept_loop(listener: &TcpListener, body: &[u8], remaining: &AtomicU32) {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            if handle_connection(stream, body, remaining) {
                break;
            }
        }
    }

    /// Serves `body` over HTTP on a loopback port, failing the first
    /// `failures` requests with HTTP 500.
    fn serve(body: &'static [u8], failures: u32) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let port = listener.local_addr().expect("local addr").port();
        let remaining = Arc::new(AtomicU32::new(failures));
        let handle = std::thread::spawn(move || accept_loop(&listener, body, &remaining));
        (format!("http://127.0.0.1:{port}"), handle)
    }

    fn stop_server(base: &str, handle: std::thread::JoinHandle<()>) {
        let _ = reqwest::blocking::get(format!("{base}/stop"));
        let _ = handle.join();
    }

    fn short_policy() -> FetchPolicy {
        FetchPolicy {
            timeout: Duration::from_secs(2),
            retries: 0,
            ..FetchPolicy::default()
        }
    }

    #[test]
    fn fetch_offline_policy_rejects_before_connecting() {
        let reference = ImageReference::parse(&format!(
            "http://127.0.0.1:1/image.tar@sha256:{BODY_DIGEST}"
        ))
        .expect("parse");
        let policy = FetchPolicy {
            offline: true,
            ..short_policy()
        };
        let dir = tempfile::tempdir().expect("tempdir");
        let error = fetch_remote(&reference, &policy, &dir.path().join("out.tar"))
            .expect_err("offline must fail");
        assert!(error.to_string().contains("offline"));
    }

    #[test]
    fn fetch_without_pinned_digest_rejected() {
        let reference = ImageReference::parse("https://example.test/image.tar").expect("parse");
        let dir = tempfile::tempdir().expect("tempdir");
        let error = fetch_remote(&reference, &short_policy(), &dir.path().join("out.tar"))
            .expect_err("missing digest must fail");
        assert!(error.to_string().contains("digest"));
    }

    #[test]
    fn fetch_success_verifies_digest_and_writes_file() {
        let (base, handle) = serve(BODY, 0);
        let reference = ImageReference::parse(&format!("{base}/image.tar@sha256:{BODY_DIGEST}"))
            .expect("parse");
        let dir = tempfile::tempdir().expect("tempdir");
        let destination = dir.path().join("out.tar");

        let digest =
            fetch_remote(&reference, &short_policy(), &destination).expect("fetch succeeds");

        assert_eq!(digest.as_hex(), BODY_DIGEST);
        assert_eq!(std::fs::read(&destination).expect("read"), BODY);
        stop_server(&base, handle);
    }

    #[test]
    fn fetch_digest_mismatch_deletes_download() {
        let (base, handle) = serve(BODY, 0);
        let wrong = "0".repeat(64);
        let reference =
            ImageReference::parse(&format!("{base}/image.tar@sha256:{wrong}")).expect("parse");
        let dir = tempfile::tempdir().expect("tempdir");
        let destination = dir.path().join("out.tar");

        let error = fetch_remote(&reference, &short_policy(), &destination)
            .expect_err("mismatch must fail");

        assert!(matches!(error, ContainustError::HashMismatch { .. }));
        assert!(!destination.exists());
        stop_server(&base, handle);
    }

    #[test]
    fn fetch_retries_after_server_error() {
        let (base, handle) = serve(BODY, 1);
        let reference = ImageReference::parse(&format!("{base}/image.tar@sha256:{BODY_DIGEST}"))
            .expect("parse");
        let policy = FetchPolicy {
            retries: 1,
            ..short_policy()
        };
        let dir = tempfile::tempdir().expect("tempdir");
        let destination = dir.path().join("out.tar");

        let _ = fetch_remote(&reference, &policy, &destination).expect("retry succeeds");

        assert_eq!(std::fs::read(&destination).expect("read"), BODY);
        stop_server(&base, handle);
    }

    #[test]
    fn fetch_size_limit_rejects_oversized_payload() {
        let (base, handle) = serve(BODY, 0);
        let reference = ImageReference::parse(&format!("{base}/image.tar@sha256:{BODY_DIGEST}"))
            .expect("parse");
        let policy = FetchPolicy {
            max_bytes: 4,
            ..short_policy()
        };
        let dir = tempfile::tempdir().expect("tempdir");

        let error = fetch_remote(&reference, &policy, &dir.path().join("out.tar"))
            .expect_err("size limit must fail");

        assert!(error.to_string().contains("limit"));
        stop_server(&base, handle);
    }
}
