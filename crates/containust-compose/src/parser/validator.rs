//! Static analysis and validation of the parsed AST.
//!
//! Checks for undefined references, duplicate names, and
//! missing required properties before the composition is deployed.

use std::collections::HashSet;

use containust_common::error::{ContainustError, Result};

use super::ast::CompositionFile;

/// Validates a parsed composition file for semantic correctness.
///
/// # Checks performed
///
/// 1. No duplicate component names.
/// 2. Every CONNECT source and target references a defined component.
/// 3. Components without a FROM template must declare an `image` property.
///
/// # Errors
///
/// Returns an error if any semantic check fails.
pub fn validate(file: &CompositionFile) -> Result<()> {
    tracing::info!("validating composition file");
    check_duplicate_components(file)?;
    check_connection_references(file)?;
    check_image_required(file)?;
    Ok(())
}

fn check_duplicate_components(file: &CompositionFile) -> Result<()> {
    let mut seen = HashSet::new();
    for comp in &file.components {
        if !seen.insert(&comp.name) {
            return Err(ContainustError::Config {
                message: format!("duplicate component name: \"{}\"", comp.name),
            });
        }
    }
    Ok(())
}

fn check_connection_references(file: &CompositionFile) -> Result<()> {
    let names: HashSet<&str> = file.components.iter().map(|c| c.name.as_str()).collect();

    for conn in &file.connections {
        if !names.contains(conn.from.as_str()) {
            return Err(ContainustError::NotFound {
                kind: "component",
                id: format!("CONNECT source \"{}\" is not defined", conn.from),
            });
        }
        if !names.contains(conn.to.as_str()) {
            return Err(ContainustError::NotFound {
                kind: "component",
                id: format!("CONNECT target \"{}\" is not defined", conn.to),
            });
        }
    }
    Ok(())
}

fn check_image_required(file: &CompositionFile) -> Result<()> {
    for comp in &file.components {
        if comp.from_template.is_none() && comp.image.is_none() {
            return Err(ContainustError::Config {
                message: format!(
                    "component \"{}\" has no FROM template and no image property",
                    comp.name
                ),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::{ComponentDecl, ConnectionDecl};

    fn make_component(name: &str, image: Option<&str>) -> ComponentDecl {
        ComponentDecl {
            name: name.into(),
            image: image.map(Into::into),
            ..ComponentDecl::default()
        }
    }

    fn make_from_component(name: &str, template: &str) -> ComponentDecl {
        ComponentDecl {
            name: name.into(),
            from_template: Some(template.into()),
            ..ComponentDecl::default()
        }
    }

    #[test]
    fn validate_empty_file_succeeds() {
        let file = CompositionFile::default();
        assert!(validate(&file).is_ok());
    }

    #[test]
    fn validate_valid_file_succeeds() {
        let file = CompositionFile {
            imports: Vec::new(),
            components: vec![
                make_component("api", Some("api:latest")),
                make_component("db", Some("postgres:15")),
            ],
            connections: vec![ConnectionDecl {
                from: "api".into(),
                to: "db".into(),
            }],
        };
        assert!(validate(&file).is_ok());
    }

    #[test]
    fn validate_duplicate_component_name_fails() {
        let file = CompositionFile {
            imports: Vec::new(),
            components: vec![
                make_component("api", Some("img1")),
                make_component("api", Some("img2")),
            ],
            connections: Vec::new(),
        };
        let err = validate(&file).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("duplicate component name"), "got: {msg}");
    }

    #[test]
    fn validate_undefined_connect_source_fails() {
        let file = CompositionFile {
            imports: Vec::new(),
            components: vec![make_component("db", Some("postgres"))],
            connections: vec![ConnectionDecl {
                from: "ghost".into(),
                to: "db".into(),
            }],
        };
        let err = validate(&file).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("ghost"), "got: {msg}");
    }

    #[test]
    fn validate_undefined_connect_target_fails() {
        let file = CompositionFile {
            imports: Vec::new(),
            components: vec![make_component("api", Some("api"))],
            connections: vec![ConnectionDecl {
                from: "api".into(),
                to: "ghost".into(),
            }],
        };
        let err = validate(&file).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("ghost"), "got: {msg}");
    }

    #[test]
    fn validate_missing_image_without_from_fails() {
        let file = CompositionFile {
            imports: Vec::new(),
            components: vec![ComponentDecl {
                name: "broken".into(),
                ..ComponentDecl::default()
            }],
            connections: Vec::new(),
        };
        let err = validate(&file).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("no FROM template and no image"), "got: {msg}");
    }

    #[test]
    fn validate_from_template_without_image_succeeds() {
        let file = CompositionFile {
            imports: Vec::new(),
            components: vec![make_from_component("db", "pg")],
            connections: Vec::new(),
        };
        assert!(validate(&file).is_ok());
    }

    #[test]
    fn validate_multiple_connections_to_same_target() {
        let file = CompositionFile {
            imports: Vec::new(),
            components: vec![
                make_component("a", Some("img")),
                make_component("b", Some("img")),
                make_component("c", Some("img")),
            ],
            connections: vec![
                ConnectionDecl {
                    from: "a".into(),
                    to: "c".into(),
                },
                ConnectionDecl {
                    from: "b".into(),
                    to: "c".into(),
                },
            ],
        };
        assert!(validate(&file).is_ok());
    }
}
