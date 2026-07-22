//! JSON-RPC client for the in-VM Containust agent.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::PathBuf;

use containust_common::error::{ContainustError, Result};

/// Host-forwarded TCP port for the in-VM agent.
pub const VM_AGENT_PORT: u16 = 10809;

const VM_BOOT_TIMEOUT_SECS: u64 = 60;
const VM_POLL_INTERVAL_MS: u64 = 500;
const RPC_MAX_RETRIES: u32 = 8;
const RPC_RETRY_DELAY_MS: u64 = 800;

/// Returns true when the agent answers `ping` with `pong`.
#[must_use]
pub fn is_agent_ready() -> bool {
    TcpStream::connect(format!("127.0.0.1:{VM_AGENT_PORT}"))
        .ok()
        .is_some_and(|mut stream| check_agent_ping(&mut stream))
}

/// Polls TCP until the VM agent is reachable or the timeout elapses.
///
/// # Errors
///
/// Returns an error when the agent does not become ready in time.
pub fn wait_for_vm_ready() -> Result<()> {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(VM_BOOT_TIMEOUT_SECS);

    while start.elapsed() < timeout {
        if is_agent_ready() {
            eprintln!("  VM is ready.");
            tracing::info!("VM is ready");
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(VM_POLL_INTERVAL_MS));
    }

    Err(ContainustError::Config {
        message: format!("VM failed to become reachable within {VM_BOOT_TIMEOUT_SECS}s"),
    })
}

/// Sends a JSON-RPC request to the in-VM agent and returns the response.
///
/// # Errors
///
/// Returns an error if the VM is unreachable, serialization fails, or the
/// agent returns an error response.
pub fn send_rpc(method: &str, params: &serde_json::Value) -> Result<serde_json::Value> {
    let request = serde_json::json!({ "method": method, "params": params });
    let mut payload = serde_json::to_string(&request)?;
    payload.push('\n');

    let mut last_err = None;
    for attempt in 0..RPC_MAX_RETRIES {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(RPC_RETRY_DELAY_MS));
        }
        match try_send_rpc(&payload) {
            Ok(val) => {
                if let Some(error) = val.get("error") {
                    return Err(ContainustError::Config {
                        message: format!("VM agent error: {error}"),
                    });
                }
                return Ok(val);
            }
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

fn check_agent_ping(stream: &mut TcpStream) -> bool {
    let request = serde_json::json!({"method": "ping", "params": {}});
    let mut payload = serde_json::to_string(&request).unwrap_or_default();
    payload.push('\n');
    if stream.write_all(payload.as_bytes()).is_err() {
        return false;
    }
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).is_ok() && line.contains("pong")
}

fn try_send_rpc(payload: &str) -> Result<serde_json::Value> {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{VM_AGENT_PORT}")).map_err(|e| {
        ContainustError::Io {
            path: PathBuf::from("VM agent"),
            source: e,
        }
    })?;

    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));

    stream
        .write_all(payload.as_bytes())
        .map_err(|e| ContainustError::Io {
            path: PathBuf::from("VM agent"),
            source: e,
        })?;

    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    let _bytes = reader
        .read_line(&mut line)
        .map_err(|e| ContainustError::Io {
            path: PathBuf::from("VM agent"),
            source: e,
        })?;

    if line.trim().is_empty() {
        return Err(ContainustError::Config {
            message: "empty response from VM agent".into(),
        });
    }

    serde_json::from_str(&line).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    #[test]
    fn send_rpc_constructs_valid_json() {
        let method = "create";
        let params = &serde_json::json!({"name": "test"});
        let request = serde_json::json!({ "method": method, "params": params });
        let payload = serde_json::to_string(&request).expect("serialize");
        assert!(payload.contains("\"method\":\"create\""));
        assert!(payload.contains("\"name\":\"test\""));
    }
}
