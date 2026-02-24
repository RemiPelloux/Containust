//! Domain primitive types used across the Containust workspace.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Unique identifier for a container instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContainerId(String);

impl ContainerId {
    /// Creates a new container ID from a string value.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generates a random container ID.
    #[must_use]
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Returns the inner string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContainerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a container image.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ImageId(String);

impl ImageId {
    /// Creates a new image ID from a string value.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the inner string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ImageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// SHA-256 hash digest used for content verification.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Sha256Hash(String);

impl Sha256Hash {
    /// Creates a hash from a hex-encoded string.
    ///
    /// # Errors
    ///
    /// Returns an error if the input is not a valid 64-character hex string.
    pub fn from_hex(hex: impl Into<String>) -> crate::error::Result<Self> {
        let hex = hex.into();
        if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(crate::error::ContainustError::Config {
                message: format!("invalid SHA-256 hex string: {hex}"),
            });
        }
        Ok(Self(hex))
    }

    /// Returns the hex-encoded hash string.
    #[must_use]
    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sha256Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sha256:{}", self.0)
    }
}

/// Resource limits for a container.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// CPU shares (relative weight).
    pub cpu_shares: Option<u64>,
    /// Memory limit in bytes.
    pub memory_bytes: Option<u64>,
    /// I/O weight (1-10000).
    pub io_weight: Option<u16>,
}

/// Lifecycle state of a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContainerState {
    /// Container has been created but not yet started.
    Created,
    /// Container is actively running.
    Running,
    /// Container has been stopped.
    Stopped,
    /// Container encountered a fatal error.
    Failed,
}

impl fmt::Display for ContainerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::Running => write!(f, "running"),
            Self::Stopped => write!(f, "stopped"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_id_new_stores_value() {
        let id = ContainerId::new("abc-123");
        assert_eq!(id.as_str(), "abc-123");
    }

    #[test]
    fn container_id_generate_produces_unique_ids() {
        let a = ContainerId::generate();
        let b = ContainerId::generate();
        assert_ne!(a, b);
    }

    #[test]
    fn container_id_display_matches_inner() {
        let id = ContainerId::new("test-id");
        assert_eq!(format!("{id}"), "test-id");
    }

    #[test]
    fn container_id_serialization_roundtrip() {
        let id = ContainerId::new("serial-test");
        let json = serde_json::to_string(&id).expect("serialize");
        let back: ContainerId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(id, back);
    }

    #[test]
    fn image_id_new_and_display() {
        let id = ImageId::new("img-42");
        assert_eq!(id.as_str(), "img-42");
        assert_eq!(format!("{id}"), "img-42");
    }

    #[test]
    fn sha256_hash_valid_hex_accepted() {
        let hex = "a".repeat(64);
        let hash = Sha256Hash::from_hex(hex.clone()).expect("valid hex");
        assert_eq!(hash.as_hex(), hex);
    }

    #[test]
    fn sha256_hash_wrong_length_rejected() {
        assert!(Sha256Hash::from_hex("abcdef").is_err());
    }

    #[test]
    fn sha256_hash_non_hex_chars_rejected() {
        let bad = format!("{}zz", "a".repeat(62));
        assert!(Sha256Hash::from_hex(bad).is_err());
    }

    #[test]
    fn sha256_hash_display_prefixed() {
        let hex = "b".repeat(64);
        let hash = Sha256Hash::from_hex(hex.clone()).expect("valid");
        assert_eq!(format!("{hash}"), format!("sha256:{hex}"));
    }

    #[test]
    fn resource_limits_default_all_none() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.cpu_shares, None);
        assert_eq!(limits.memory_bytes, None);
        assert_eq!(limits.io_weight, None);
    }

    #[test]
    fn container_state_display_values() {
        assert_eq!(format!("{}", ContainerState::Created), "created");
        assert_eq!(format!("{}", ContainerState::Running), "running");
        assert_eq!(format!("{}", ContainerState::Stopped), "stopped");
        assert_eq!(format!("{}", ContainerState::Failed), "failed");
    }

    #[test]
    fn container_state_is_copy() {
        let state = ContainerState::Running;
        let copied = state;
        assert_eq!(state, copied);
    }

    #[test]
    fn container_state_serialization_roundtrip() {
        let state = ContainerState::Running;
        let json = serde_json::to_string(&state).expect("serialize");
        let back: ContainerState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(state, back);
    }
}
