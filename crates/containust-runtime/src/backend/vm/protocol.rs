//! Versioned line-delimited JSON RPC contract for the VM agent.

use containust_common::error::{ContainustError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Protocol version spoken by host and agent.
pub const PROTOCOL_VERSION: u32 = 1;

/// Maximum encoded request size (bytes), including the trailing newline.
pub const MAX_REQUEST_BYTES: usize = 64 * 1024;

/// Maximum response line size (bytes), including the trailing newline.
pub const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

/// Per-socket read/write timeout for a single RPC attempt.
pub const RPC_IO_TIMEOUT_SECS: u64 = 30;

/// Wire request envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    /// Protocol version (`1`).
    pub v: u32,
    /// Opaque request correlator echoed by the agent.
    pub id: String,
    /// Method name (`ping`, `create`, …).
    pub method: String,
    /// Method parameters object.
    pub params: Value,
}

/// Wire response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    /// Protocol version (`1`).
    pub v: u32,
    /// Must match the request `id`.
    pub id: String,
    /// Success payload when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error string when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Builds a newline-terminated request payload and returns `(id, payload)`.
///
/// # Errors
///
/// Returns an error when serialization fails or the payload exceeds
/// [`MAX_REQUEST_BYTES`].
pub fn encode_request(method: &str, params: &Value) -> Result<(String, String)> {
    let id = new_request_id();
    let request = RpcRequest {
        v: PROTOCOL_VERSION,
        id: id.clone(),
        method: method.to_string(),
        params: params.clone(),
    };
    let mut payload = serde_json::to_string(&request)?;
    payload.push('\n');
    if payload.len() > MAX_REQUEST_BYTES {
        return Err(ContainustError::Config {
            message: format!(
                "VM RPC request exceeds {MAX_REQUEST_BYTES} bytes (got {})",
                payload.len()
            ),
        });
    }
    Ok((id, payload))
}

/// Parses and validates a response line against the expected request id.
///
/// # Errors
///
/// Returns an error on oversized input, JSON failure, version/id mismatch,
/// missing result/error, or an agent error string.
pub fn decode_response(line: &str, expected_id: &str) -> Result<Value> {
    if line.len() > MAX_RESPONSE_BYTES {
        return Err(ContainustError::Config {
            message: format!("VM RPC response exceeds {MAX_RESPONSE_BYTES} bytes"),
        });
    }
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err(ContainustError::Config {
            message: "empty response from VM agent".into(),
        });
    }
    let response: RpcResponse = serde_json::from_str(trimmed)?;
    if response.v != PROTOCOL_VERSION {
        return Err(ContainustError::Config {
            message: format!(
                "VM agent protocol version mismatch: expected {PROTOCOL_VERSION}, got {}",
                response.v
            ),
        });
    }
    if response.id != expected_id {
        return Err(ContainustError::Config {
            message: format!(
                "VM agent response id mismatch: expected {expected_id}, got {}",
                response.id
            ),
        });
    }
    match (response.result, response.error) {
        (Some(result), None) => Ok(serde_json::json!({ "result": result })),
        (None, Some(error)) => Err(ContainustError::Config {
            message: format!("VM agent error: {error}"),
        }),
        (Some(_), Some(_)) => Err(ContainustError::Config {
            message: "VM agent response has both result and error".into(),
        }),
        (None, None) => Err(ContainustError::Config {
            message: "VM agent response missing result and error".into(),
        }),
    }
}

fn new_request_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mixed = nanos ^ (u128::from(std::process::id()) << 32);
    format!("{mixed:032x}")[..16].to_string()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn encode_request_includes_version_and_id() {
        let (id, payload) = encode_request("ping", &serde_json::json!({})).expect("encode");
        assert_eq!(id.len(), 16);
        assert!(payload.contains("\"v\":1"));
        assert!(payload.contains(&format!("\"id\":\"{id}\"")));
        assert!(payload.contains("\"method\":\"ping\""));
        assert!(payload.ends_with('\n'));
    }

    #[test]
    fn encode_request_rejects_oversized_payload() {
        let huge = "x".repeat(MAX_REQUEST_BYTES);
        let err = encode_request("create", &serde_json::json!({ "name": huge })).expect_err("size");
        assert!(err.to_string().contains("exceeds"));
    }

    #[test]
    fn decode_response_accepts_matching_result() {
        let line = r#"{"v":1,"id":"abcd1234abcd1234","result":"pong"}"#;
        let value = decode_response(line, "abcd1234abcd1234").expect("decode");
        assert_eq!(value["result"], "pong");
    }

    #[test]
    fn decode_response_rejects_id_mismatch() {
        let line = r#"{"v":1,"id":"other","result":"pong"}"#;
        let err = decode_response(line, "abcd1234abcd1234").expect_err("id");
        assert!(err.to_string().contains("id mismatch"));
    }

    #[test]
    fn decode_response_rejects_version_mismatch() {
        let line = r#"{"v":99,"id":"abcd1234abcd1234","result":"pong"}"#;
        let err = decode_response(line, "abcd1234abcd1234").expect_err("v");
        assert!(err.to_string().contains("version mismatch"));
    }

    #[test]
    fn decode_response_surfaces_agent_error() {
        let line = r#"{"v":1,"id":"abcd1234abcd1234","error":"not found"}"#;
        let err = decode_response(line, "abcd1234abcd1234").expect_err("error");
        assert!(err.to_string().contains("not found"));
    }
}
