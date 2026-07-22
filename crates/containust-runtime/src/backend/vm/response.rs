//! Parse helpers for VM agent JSON responses.

use containust_common::error::{ContainustError, Result};
use containust_common::types::ContainerId;

use super::super::ContainerInfo;
use crate::exec::ExecOutput;

/// Safely converts a `u64` to `u32`, returning an error on overflow.
pub fn truncate_u64_to_u32(value: u64) -> Result<u32> {
    u32::try_from(value).map_err(|_| ContainustError::Config {
        message: format!("PID value {value} exceeds u32 range"),
    })
}

/// Extracts `ExecOutput` fields from a VM agent response.
#[must_use]
pub fn parse_exec_output(response: &serde_json::Value) -> ExecOutput {
    let result = response.get("result").cloned().unwrap_or_default();
    let stdout = result
        .get("stdout")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let stderr = result
        .get("stderr")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let raw_code = result
        .get("exit_code")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(-1);
    let exit_code = i32::try_from(raw_code).unwrap_or(-1);
    ExecOutput {
        stdout,
        stderr,
        exit_code,
    }
}

/// Parses a JSON value from the VM agent into a `ContainerInfo`.
#[must_use]
pub fn parse_container_info(value: &serde_json::Value) -> Option<ContainerInfo> {
    let pid_u64 = value.get("pid").and_then(serde_json::Value::as_u64);
    let pid = pid_u64.and_then(|v| u32::try_from(v).ok());
    Some(ContainerInfo {
        id: ContainerId::new(value.get("id")?.as_str()?),
        name: value.get("name")?.as_str()?.to_string(),
        state: value.get("state")?.as_str()?.to_string(),
        pid,
        image: value.get("image")?.as_str()?.to_string(),
        created_at: value.get("created_at")?.as_str()?.to_string(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn truncate_u64_to_u32_within_range() {
        assert_eq!(truncate_u64_to_u32(42).expect("ok"), 42u32);
    }

    #[test]
    fn truncate_u64_to_u32_overflow_returns_error() {
        assert!(truncate_u64_to_u32(u64::from(u32::MAX) + 1).is_err());
    }

    #[test]
    fn parse_exec_output_with_all_fields() {
        let response = serde_json::json!({
            "result": {
                "stdout": "hello world",
                "stderr": "warning",
                "exit_code": 0
            }
        });
        let output = parse_exec_output(&response);
        assert_eq!(output.stdout, "hello world");
        assert_eq!(output.stderr, "warning");
        assert_eq!(output.exit_code, 0);
    }

    #[test]
    fn parse_exec_output_missing_fields_uses_defaults() {
        let output = parse_exec_output(&serde_json::json!({}));
        assert_eq!(output.stdout, "");
        assert_eq!(output.stderr, "");
        assert_eq!(output.exit_code, -1);
    }

    #[test]
    fn parse_container_info_valid_json() {
        let value = serde_json::json!({
            "id": "test-123",
            "name": "my-app",
            "state": "running",
            "pid": 1234,
            "image": "file:///app",
            "created_at": "2024-01-01T00:00:00Z"
        });
        let info = parse_container_info(&value).expect("should parse");
        assert_eq!(info.id, ContainerId::new("test-123"));
        assert_eq!(info.pid, Some(1234));
    }

    #[test]
    fn parse_container_info_missing_fields_returns_none() {
        assert!(parse_container_info(&serde_json::json!({ "id": "x" })).is_none());
    }
}
