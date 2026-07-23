//! # containust-compose
//!
//! Parser and resolver for the `.ctst` composition language.
//!
//! Handles:
//! - **Parser**: Lexing, AST construction, and validation of `.ctst` files.
//! - **Graph**: Dependency graph construction and topological resolution.
//! - **Resolver**: Auto-wiring of environment variables between components.
//! - **Component**: COMPONENT block definitions and parameterization.
//! - **Import**: IMPORT resolution from files and network.
//! - **Distroless**: Binary dependency analysis for minimal images.

#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

pub mod component;
pub mod distroless;
pub mod graph;
pub mod import;
pub mod parser;
pub mod resolver;

use containust_common::error::{ContainustError, Result};

/// Rejects network-backed imports and images for offline execution.
///
/// # Errors
///
/// Returns an error when the composition contains an HTTP(S) source.
pub fn validate_offline(file: &parser::ast::CompositionFile) -> Result<()> {
    let remote_import = file
        .imports
        .iter()
        .map(|import| import.source.as_str())
        .find(|source| is_remote_source(source));
    if let Some(source) = remote_import {
        return Err(ContainustError::Config {
            message: format!("offline mode rejects remote source: {source}"),
        });
    }
    if let Some(source) = file
        .components
        .iter()
        .filter_map(|component| component.image.as_deref())
        .find(|source| !is_local_image(source))
    {
        return Err(ContainustError::Config {
            message: format!(
                "offline mode requires a file://, tar://, image://, or cached preset:// image: \
                 {source}"
            ),
        });
    }
    Ok(())
}

fn is_remote_source(source: &str) -> bool {
    source.starts_with("http://") || source.starts_with("https://") || source.starts_with("oci://")
}

fn is_local_image(source: &str) -> bool {
    // preset:// is allowed offline at composition time; import fails closed
    // if the curated layer is not already in the local store.
    source.starts_with("file://")
        || source.starts_with("tar://")
        || source.starts_with("image://")
        || source.starts_with("preset://")
}

#[cfg(test)]
mod example_tests {
    use std::path::Path;

    #[test]
    fn all_bundled_examples_parse_and_validate() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
        let mut checked = 0;
        for entry in std::fs::read_dir(dir).expect("examples directory") {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("ctst") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read example");
            let result = crate::parser::parse_ctst(&text);
            assert!(
                result.is_ok(),
                "example {} failed to parse: {:?}",
                path.display(),
                result.err()
            );
            checked += 1;
        }
        assert!(checked > 0, "no examples found");
    }
}

#[cfg(test)]
mod offline_tests {
    use super::*;
    use crate::parser::ast::{ComponentDecl, CompositionFile, ImportDecl};

    #[test]
    fn offline_accepts_local_sources() {
        let file = CompositionFile {
            exposes: Vec::new(),
            imports: vec![ImportDecl {
                source: "templates/base.ctst".into(),
                alias: None,
            }],
            components: vec![ComponentDecl {
                image: Some("file:///images/app".into()),
                ..ComponentDecl::default()
            }],
            connections: Vec::new(),
        };
        assert!(validate_offline(&file).is_ok());
    }

    #[test]
    fn offline_accepts_catalog_image() {
        let file = CompositionFile {
            exposes: Vec::new(),
            components: vec![ComponentDecl {
                image: Some(
                    "image://web@sha256:0000000000000000000000000000000000000000000000000000000000000000"
                        .into(),
                ),
                ..ComponentDecl::default()
            }],
            ..CompositionFile::default()
        };
        assert!(validate_offline(&file).is_ok());
    }

    #[test]
    fn offline_accepts_preset_image_at_composition_level() {
        let file = CompositionFile {
            exposes: Vec::new(),
            components: vec![ComponentDecl {
                image: Some("preset://alpine".into()),
                ..ComponentDecl::default()
            }],
            ..CompositionFile::default()
        };
        assert!(validate_offline(&file).is_ok());
    }

    #[test]
    fn offline_rejects_remote_image() {
        let file = CompositionFile {
            exposes: Vec::new(),
            components: vec![ComponentDecl {
                image: Some("https://example.test/app.tar".into()),
                ..ComponentDecl::default()
            }],
            ..CompositionFile::default()
        };
        assert!(validate_offline(&file).is_err());
    }

    #[test]
    fn offline_rejects_oci_image() {
        let file = CompositionFile {
            exposes: Vec::new(),
            components: vec![ComponentDecl {
                image: Some("oci://alpine:3.21".into()),
                ..ComponentDecl::default()
            }],
            ..CompositionFile::default()
        };
        assert!(validate_offline(&file).is_err());
    }

    #[test]
    fn offline_rejects_remote_import() {
        let file = CompositionFile {
            exposes: Vec::new(),
            imports: vec![ImportDecl {
                source: "http://example.test/base.ctst".into(),
                alias: None,
            }],
            ..CompositionFile::default()
        };
        assert!(validate_offline(&file).is_err());
    }

    #[test]
    fn offline_rejects_registry_style_image() {
        let file = CompositionFile {
            exposes: Vec::new(),
            components: vec![ComponentDecl {
                image: Some("alpine:3.21".into()),
                ..ComponentDecl::default()
            }],
            ..CompositionFile::default()
        };
        assert!(validate_offline(&file).is_err());
    }
}
