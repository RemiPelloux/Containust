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

/// Restart policy applied when a container's process exits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicy {
    /// Never restart automatically (default).
    #[default]
    Never,
    /// Restart only after an abnormal exit.
    OnFailure,
    /// Always restart after any exit.
    Always,
}

impl RestartPolicy {
    /// Parses the `.ctst` restart property value.
    ///
    /// # Errors
    ///
    /// Returns the offending value when it is not one of
    /// `never`, `on-failure`, or `always`.
    pub fn parse(value: &str) -> std::result::Result<Self, String> {
        match value.trim() {
            "never" | "no" => Ok(Self::Never),
            "on-failure" => Ok(Self::OnFailure),
            "always" => Ok(Self::Always),
            other => Err(format!(
                "invalid restart policy '{other}' (expected never, on-failure, or always)"
            )),
        }
    }
}

impl fmt::Display for RestartPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Never => write!(f, "never"),
            Self::OnFailure => write!(f, "on-failure"),
            Self::Always => write!(f, "always"),
        }
    }
}

/// Health probe configuration attached to a container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthcheckSpec {
    /// Command executed inside the container.
    pub command: Vec<String>,
    /// Seconds between probe executions.
    pub interval_secs: u64,
    /// Probe timeout in seconds.
    pub timeout_secs: u64,
    /// Consecutive failures before the container is unhealthy.
    pub retries: u32,
    /// Grace period after start before probes count.
    pub start_period_secs: u64,
}

impl Default for HealthcheckSpec {
    fn default() -> Self {
        Self {
            command: Vec::new(),
            interval_secs: 30,
            timeout_secs: 30,
            retries: 3,
            start_period_secs: 0,
        }
    }
}

/// Observed health of a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HealthState {
    /// Probes have not yet produced a verdict.
    Starting,
    /// The most recent probe succeeded.
    Healthy,
    /// The failure threshold was exceeded.
    Unhealthy,
}

impl fmt::Display for HealthState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Starting => write!(f, "starting"),
            Self::Healthy => write!(f, "healthy"),
            Self::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

/// Host-to-container port publish mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PortMapping {
    /// Port bound on the host.
    pub host: u16,
    /// Port the container process listens on.
    pub container: u16,
}

impl PortMapping {
    /// Identity mapping (host port equals container port).
    #[must_use]
    pub const fn identity(port: u16) -> Self {
        Self {
            host: port,
            container: port,
        }
    }

    /// Returns true when host and container ports differ.
    #[must_use]
    pub const fn is_remap(self) -> bool {
        self.host != self.container
    }
}

/// Persistent health probe bookkeeping for one container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthRecord {
    /// Latest verdict.
    pub state: HealthState,
    /// Consecutive probe failures.
    pub consecutive_failures: u32,
    /// ISO-8601 timestamp of the last executed probe.
    pub last_probe_at: Option<String>,
}

impl Default for HealthRecord {
    fn default() -> Self {
        Self {
            state: HealthState::Starting,
            consecutive_failures: 0,
            last_probe_at: None,
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

    #[test]
    fn port_mapping_identity_and_remap() {
        let id = PortMapping::identity(8080);
        assert!(!id.is_remap());
        let remap = PortMapping {
            host: 8080,
            container: 80,
        };
        assert!(remap.is_remap());
    }
}
