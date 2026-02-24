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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_serialization_roundtrip() {
        let template = ComponentTemplate {
            name: "postgres".into(),
            image: "file:///opt/postgres".into(),
            defaults: vec![("POSTGRES_USER".into(), "admin".into())],
            required_params: vec!["POSTGRES_PASSWORD".into()],
        };
        let json = serde_json::to_string(&template).expect("serialize");
        let back: ComponentTemplate = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.name, "postgres");
        assert_eq!(back.defaults.len(), 1);
        assert_eq!(back.required_params, vec!["POSTGRES_PASSWORD"]);
    }

    #[test]
    fn template_clone_is_independent() {
        let template = ComponentTemplate {
            name: "base".into(),
            image: "file:///base".into(),
            defaults: vec![],
            required_params: vec![],
        };
        let cloned = template.clone();
        assert_eq!(template.name, cloned.name);
        assert_eq!(template.image, cloned.image);
    }
}
