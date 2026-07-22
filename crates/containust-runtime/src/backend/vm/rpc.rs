//! TCP client for the versioned VM agent RPC contract.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

use containust_common::error::{ContainustError, Result};

use super::protocol::{MAX_RESPONSE_BYTES, RPC_IO_TIMEOUT_SECS, decode_response, encode_request};

/// Host-forwarded TCP port for the in-VM agent.
pub const VM_AGENT_PORT: u16 = 10809;

/// Default wait for the guest agent after QEMU start (cold CI boots need longer).
const VM_BOOT_TIMEOUT_DEFAULT_SECS: u64 = 180;
const VM_POLL_INTERVAL_MS: u64 = 500;
const RPC_MAX_RETRIES: u32 = 8;
const RPC_RETRY_DELAY_MS: u64 = 800;

fn boot_timeout_secs() -> u64 {
    parse_boot_timeout(
        std::env::var("CONTAINUST_VM_BOOT_TIMEOUT_SECS")
            .ok()
            .as_deref(),
    )
}

fn parse_boot_timeout(raw: Option<&str>) -> u64 {
    raw.and_then(|value| value.parse().ok())
        .filter(|secs| *secs > 0)
        .unwrap_or(VM_BOOT_TIMEOUT_DEFAULT_SECS)
}

/// Returns true when the agent answers a versioned `ping` with `pong`.
#[must_use]
pub fn is_agent_ready() -> bool {
    send_rpc("ping", &serde_json::json!({}))
        .is_ok_and(|value| value.get("result").and_then(serde_json::Value::as_str) == Some("pong"))
}

/// Polls TCP until the VM agent is reachable or the timeout elapses.
///
/// # Errors
///
/// Returns an error when the agent does not become ready in time.
pub fn wait_for_vm_ready() -> Result<()> {
    let start = std::time::Instant::now();
    let timeout_secs = boot_timeout_secs();
    let timeout = Duration::from_secs(timeout_secs);

    while start.elapsed() < timeout {
        if is_agent_ready() {
            eprintln!("  VM is ready.");
            tracing::info!("VM is ready");
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(VM_POLL_INTERVAL_MS));
    }

    Err(ContainustError::Config {
        message: format!("VM failed to become reachable within {timeout_secs}s"),
    })
}

/// Sends a versioned RPC request and returns `{ "result": ... }`.
///
/// # Errors
///
/// Returns an error if encoding, transport, validation, or the agent fails.
pub fn send_rpc(method: &str, params: &serde_json::Value) -> Result<serde_json::Value> {
    let (request_id, payload) = encode_request(method, params)?;
    let mut last_err = None;
    for attempt in 0..RPC_MAX_RETRIES {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(RPC_RETRY_DELAY_MS));
        }
        match try_send_rpc(&payload, &request_id) {
            Ok(val) => return Ok(val),
            Err(e) => {
                tracing::debug!(attempt, error = %e, "RPC attempt failed, retrying");
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| ContainustError::Config {
        message: "RPC failed after all retries".into(),
    }))
}

fn try_send_rpc(payload: &str, expected_id: &str) -> Result<serde_json::Value> {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{VM_AGENT_PORT}")).map_err(|e| {
        ContainustError::Io {
            path: PathBuf::from("VM agent"),
            source: e,
        }
    })?;
    let timeout = Duration::from_secs(RPC_IO_TIMEOUT_SECS);
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| ContainustError::Io {
            path: PathBuf::from("VM agent"),
            source: e,
        })?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|e| ContainustError::Io {
            path: PathBuf::from("VM agent"),
            source: e,
        })?;

    stream
        .write_all(payload.as_bytes())
        .map_err(|e| ContainustError::Io {
            path: PathBuf::from("VM agent"),
            source: e,
        })?;

    let line = read_bounded_line(&mut stream)?;
    decode_response(&line, expected_id)
}

fn read_bounded_line(stream: &mut TcpStream) -> Result<String> {
    let mut buf = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) if byte[0] == b'\n' => break,
            Ok(_) => {
                if buf.len() >= MAX_RESPONSE_BYTES {
                    return Err(ContainustError::Config {
                        message: format!(
                            "VM RPC response exceeds {MAX_RESPONSE_BYTES} bytes before newline"
                        ),
                    });
                }
                buf.push(byte[0]);
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => {
                return Err(ContainustError::Io {
                    path: PathBuf::from("VM agent"),
                    source: error,
                });
            }
        }
    }
    String::from_utf8(buf).map_err(|e| ContainustError::Config {
        message: format!("VM agent response is not valid UTF-8: {e}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use crate::backend::vm::protocol::{decode_response, encode_request};

    fn read_until_newline(stream: &mut TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut byte = [0_u8; 1];
        while stream.read_exact(&mut byte).is_ok() && byte[0] != b'\n' {
            request.push(byte[0]);
        }
        request
    }

    #[test]
    fn parse_boot_timeout_defaults_and_overrides() {
        assert_eq!(parse_boot_timeout(None), VM_BOOT_TIMEOUT_DEFAULT_SECS);
        assert_eq!(parse_boot_timeout(Some("0")), VM_BOOT_TIMEOUT_DEFAULT_SECS);
        assert_eq!(
            parse_boot_timeout(Some("bogus")),
            VM_BOOT_TIMEOUT_DEFAULT_SECS
        );
        assert_eq!(parse_boot_timeout(Some("90")), 90);
    }

    #[test]
    fn versioned_rpc_roundtrip_over_tcp() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_until_newline(&mut stream);
            let req: serde_json::Value = serde_json::from_slice(&request).expect("request json");
            assert_eq!(req["v"], 1);
            assert_eq!(req["method"], "ping");
            let id = req["id"].as_str().expect("id");
            let response = format!(r#"{{"v":1,"id":"{id}","result":"pong"}}"#);
            stream.write_all(response.as_bytes()).expect("write");
            stream.write_all(b"\n").expect("nl");
        });

        let (id, payload) = encode_request("ping", &serde_json::json!({})).expect("encode");
        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).expect("connect");
        stream.write_all(payload.as_bytes()).expect("write");
        let line = read_bounded_line(&mut stream).expect("read");
        let value = decode_response(&line, &id).expect("decode");
        assert_eq!(value["result"], "pong");
        handle.join().expect("join");
    }
}
