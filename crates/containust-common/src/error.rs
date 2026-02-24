//! Unified error types for the Containust workspace.
//!
//! Each higher-level crate defines its own domain-specific error enum that wraps
//! these common variants when appropriate.

use std::path::PathBuf;

use thiserror::Error;

/// Top-level error type shared across the workspace.
#[derive(Debug, Error)]
pub enum ContainustError {
    /// An I/O operation failed.
    #[error("I/O error at {path}: {source}")]
    Io {
        /// Path where the I/O error occurred.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// A configuration value is invalid.
    #[error("invalid configuration: {message}")]
    Config {
        /// Description of the invalid configuration.
        message: String,
    },

    /// A required resource was not found.
    #[error("{kind} not found: {id}")]
    NotFound {
        /// Type of the missing resource.
        kind: &'static str,
        /// Identifier of the missing resource.
        id: String,
    },

    /// A hash validation failed.
    #[error("hash mismatch for {resource}: expected {expected}, got {actual}")]
    HashMismatch {
        /// Resource that failed validation.
        resource: String,
        /// Expected hash value.
        expected: String,
        /// Actual computed hash value.
        actual: String,
    },

    /// A permission or capability error.
    #[error("permission denied: {message}")]
    PermissionDenied {
        /// Description of the denied operation.
        message: String,
    },

    /// Serialization or deserialization failed.
    #[error("serialization error: {source}")]
    Serialization {
        /// Underlying serialization error.
        #[from]
        source: serde_json::Error,
    },
}

/// Convenience alias used throughout the workspace.
pub type Result<T> = std::result::Result<T, ContainustError>;
