//! Stable error codes, process exit codes, and remediation hints.

use crate::error::ContainustError;

/// Classified operator-facing error metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorClass {
    /// Catalog code such as `R001` or `I003`.
    pub code: &'static str,
    /// Process exit status for the CLI.
    pub exit_code: i32,
    /// Short remediation hint.
    pub remediation: &'static str,
}

/// Maps a domain error to a stable code, exit status, and remediation hint.
#[must_use]
pub fn classify(error: &ContainustError) -> ErrorClass {
    match error {
        ContainustError::Io { .. } => class(
            "S001",
            1,
            "Check path permissions and that parent directories exist",
        ),
        ContainustError::Config { message } => classify_config(message),
        ContainustError::NotFound { kind, .. } => classify_not_found(kind),
        ContainustError::HashMismatch { .. } => class(
            "I003",
            1,
            "Re-import the image; delete the corrupt cache entry first",
        ),
        ContainustError::PermissionDenied { .. } => class(
            "R008",
            1,
            "Run with sufficient privileges or adjust capability/namespace policy",
        ),
        ContainustError::Serialization { .. } => class(
            "S002",
            1,
            "Repair or remove the corrupt state/catalog JSON and retry",
        ),
        ContainustError::Network { message, .. } => classify_network(message),
    }
}

const fn class(code: &'static str, exit_code: i32, remediation: &'static str) -> ErrorClass {
    ErrorClass {
        code,
        exit_code,
        remediation,
    }
}

fn classify_not_found(kind: &str) -> ErrorClass {
    if kind == "QEMU binary" {
        class(
            "R010",
            1,
            "Install QEMU (macOS: brew install qemu) then retry",
        )
    } else {
        class("R001", 1, "Verify the container name/id with `ctst ps -a`")
    }
}

fn classify_network(message: &str) -> ErrorClass {
    if message.contains("offline") {
        class(
            "I004",
            1,
            "Import once online, or unset --offline / CONTAINUST_OFFLINE",
        )
    } else {
        class("I002", 1, "Check network connectivity and the image URL")
    }
}

/// Best-effort classification from an opaque error display string (e.g. anyhow).
#[must_use]
pub fn classify_message(message: &str) -> ErrorClass {
    let lower = message.to_ascii_lowercase();
    if lower.contains("offline") {
        return ErrorClass {
            code: "I004",
            exit_code: 1,
            remediation: "Import once online, or unset --offline / CONTAINUST_OFFLINE",
        };
    }
    if lower.contains("not found") {
        return ErrorClass {
            code: "R001",
            exit_code: 1,
            remediation: "Verify the container name/id with `ctst ps -a`",
        };
    }
    if lower.contains("hash mismatch") {
        return ErrorClass {
            code: "I003",
            exit_code: 1,
            remediation: "Re-import the image; delete the corrupt cache entry first",
        };
    }
    if lower.contains("qemu") {
        return ErrorClass {
            code: "R010",
            exit_code: 1,
            remediation: "Install QEMU (macOS: brew install qemu) then retry",
        };
    }
    if lower.contains("cyclic") || lower.contains("parse") || lower.contains("unexpected token") {
        return ErrorClass {
            code: "E001",
            exit_code: 2,
            remediation: "Fix the .ctst syntax and re-run `ctst plan`",
        };
    }
    if lower.contains("permission denied") {
        return ErrorClass {
            code: "R008",
            exit_code: 1,
            remediation: "Run with sufficient privileges or adjust policy",
        };
    }
    ErrorClass {
        code: "R000",
        exit_code: 1,
        remediation: "See docs/ERRORS.md for the matching code and resolution",
    }
}

fn classify_config(message: &str) -> ErrorClass {
    let lower = message.to_ascii_lowercase();
    if lower.contains("offline") {
        return ErrorClass {
            code: "I004",
            exit_code: 1,
            remediation: "Import once online, or unset --offline / CONTAINUST_OFFLINE",
        };
    }
    if lower.contains("port") {
        return ErrorClass {
            code: "R011",
            exit_code: 1,
            remediation: "Free the host port or stop the VM with `ctst vm stop` and retry",
        };
    }
    if lower.contains("protocol") || lower.contains("rpc") {
        return ErrorClass {
            code: "R012",
            exit_code: 1,
            remediation: "Run `ctst vm stop` then `ctst vm start` to rebuild the agent",
        };
    }
    ErrorClass {
        code: "R005",
        exit_code: 1,
        remediation: "Correct the configuration value named in the error message",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_offline_network_error() {
        let err = ContainustError::Network {
            url: "https://example.test".into(),
            message: "offline mode blocks remote fetch".into(),
        };
        let class = classify(&err);
        assert_eq!(class.code, "I004");
        assert!(class.remediation.contains("online"));
    }

    #[test]
    fn classify_message_parse_hint() {
        let class = classify_message("unexpected token at line 1");
        assert_eq!(class.code, "E001");
        assert_eq!(class.exit_code, 2);
    }
}
