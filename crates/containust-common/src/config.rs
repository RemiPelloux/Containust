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
