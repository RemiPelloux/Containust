//! Global configuration model for the Containust runtime.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Root configuration for the Containust runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainustConfig {
    /// Base directory for Containust state and data.
    pub data_dir: PathBuf,
    /// Path to the state index file.
    pub state_file: PathBuf,
    /// Whether offline mode is enabled (blocks all egress).
    pub offline: bool,
    /// Default resource limits applied to all containers.
    pub default_limits: crate::types::ResourceLimits,
}

impl Default for ContainustConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from(crate::constants::DEFAULT_DATA_DIR),
            state_file: PathBuf::from(crate::constants::DEFAULT_STATE_FILE),
            offline: false,
            default_limits: crate::types::ResourceLimits::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default_uses_standard_paths() {
        let cfg = ContainustConfig::default();
        assert_eq!(
            cfg.data_dir,
            PathBuf::from(crate::constants::DEFAULT_DATA_DIR)
        );
        assert_eq!(
            cfg.state_file,
            PathBuf::from(crate::constants::DEFAULT_STATE_FILE)
        );
    }

    #[test]
    fn config_default_offline_disabled() {
        let cfg = ContainustConfig::default();
        assert!(!cfg.offline);
    }

    #[test]
    fn config_default_limits_are_none() {
        let cfg = ContainustConfig::default();
        assert_eq!(cfg.default_limits, crate::types::ResourceLimits::default());
    }

    #[test]
    fn config_serialization_roundtrip() {
        let cfg = ContainustConfig::default();
        let json = serde_json::to_string(&cfg).expect("serialize");
        let back: ContainustConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.data_dir, cfg.data_dir);
        assert_eq!(back.offline, cfg.offline);
    }
}
