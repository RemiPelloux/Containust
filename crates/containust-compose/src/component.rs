//! COMPONENT block definitions and parameterization.
//!
//! Provides the runtime representation of reusable component templates
//! that can be instantiated with different parameters.

use serde::{Deserialize, Serialize};

/// A reusable component template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentTemplate {
    /// Template name.
    pub name: String,
    /// Image source URI.
    pub image: String,
    /// Default parameters (can be overridden).
    pub defaults: Vec<(String, String)>,
    /// Required parameters (must be provided).
    pub required_params: Vec<String>,
}
