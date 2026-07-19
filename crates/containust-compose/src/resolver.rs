//! Auto-wiring and environment variable injection.
//!
//! Automatically generates connection environment variables when
//! components are linked via `CONNECT` declarations.

use std::collections::HashMap;

use containust_common::error::{ContainustError, Result};

use crate::parser::ast::CompositionFile;

/// A component with its resolved environment variables.
#[derive(Debug, Clone)]
pub struct ResolvedComponent {
    /// Component name.
    pub name: String,
    /// Environment variables including auto-wired connection vars.
    pub env: Vec<(String, String)>,
}

/// Resolves connections and generates environment variables for each component.
///
/// For each `CONNECT source -> target`, the source component receives:
/// - `<TARGET_UPPER>_HOST` set to the target component name.
/// - `<TARGET_UPPER>_PORT` set to the target's port (if declared).
///
/// # Errors
///
/// Returns an error if a connection references an undefined component.
pub fn resolve_connections(file: &CompositionFile) -> Result<Vec<ResolvedComponent>> {
    let mut resolved: Vec<ResolvedComponent> = file
        .components
        .iter()
        .map(|c| ResolvedComponent {
            name: c.name.clone(),
            env: c.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        })
        .collect();
    let components: HashMap<&str, &crate::parser::ast::ComponentDecl> = file
        .components
        .iter()
        .map(|component| (component.name.as_str(), component))
        .collect();
    let resolved_indices: HashMap<&str, usize> = file
        .components
        .iter()
        .enumerate()
        .map(|(index, component)| (component.name.as_str(), index))
        .collect();

    for conn in &file.connections {
        let target = components
            .get(conn.to.as_str())
            .ok_or_else(|| ContainustError::NotFound {
                kind: "component",
                id: conn.to.clone(),
            })?;
        let source_index =
            resolved_indices
                .get(conn.from.as_str())
                .ok_or_else(|| ContainustError::NotFound {
                    kind: "component",
                    id: conn.from.clone(),
                })?;
        inject_connection_env(&mut resolved[*source_index], conn, target);
    }

    Ok(resolved)
}

fn inject_connection_env(
    source: &mut ResolvedComponent,
    conn: &crate::parser::ast::ConnectionDecl,
    target_comp: &crate::parser::ast::ComponentDecl,
) {
    let target_upper = conn.to.to_uppercase();
    let port = target_comp.port.map_or_else(String::new, |p| p.to_string());

    source
        .env
        .push((format!("{target_upper}_HOST"), conn.to.clone()));
    if !port.is_empty() {
        source.env.push((format!("{target_upper}_PORT"), port));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::parser::ast::{ComponentDecl, ConnectionDecl};

    #[test]
    fn resolve_empty_file() {
        let file = CompositionFile::default();
        let resolved = resolve_connections(&file).expect("should resolve");
        assert!(resolved.is_empty());
    }

    #[test]
    fn resolve_preserves_existing_env() {
        let mut env = BTreeMap::new();
        let _ = env.insert("KEY".into(), "value".into());
        let file = CompositionFile {
            imports: Vec::new(),
            components: vec![ComponentDecl {
                name: "svc".into(),
                image: Some("img".into()),
                env,
                ..ComponentDecl::default()
            }],
            connections: Vec::new(),
        };
        let resolved = resolve_connections(&file).expect("should resolve");
        assert_eq!(resolved.len(), 1);
        assert!(
            resolved[0]
                .env
                .iter()
                .any(|(k, v)| k == "KEY" && v == "value")
        );
    }

    #[test]
    fn resolve_injects_host_and_port() {
        let file = CompositionFile {
            imports: Vec::new(),
            components: vec![
                ComponentDecl {
                    name: "api".into(),
                    image: Some("api".into()),
                    ..ComponentDecl::default()
                },
                ComponentDecl {
                    name: "db".into(),
                    image: Some("pg".into()),
                    port: Some(5432),
                    ..ComponentDecl::default()
                },
            ],
            connections: vec![ConnectionDecl {
                from: "api".into(),
                to: "db".into(),
            }],
        };

        let resolved = resolve_connections(&file).expect("should resolve");
        let api = resolved.iter().find(|r| r.name == "api").expect("api");
        assert!(api.env.iter().any(|(k, v)| k == "DB_HOST" && v == "db"));
        assert!(api.env.iter().any(|(k, v)| k == "DB_PORT" && v == "5432"));
    }

    #[test]
    fn resolve_no_port_injects_only_host() {
        let file = CompositionFile {
            imports: Vec::new(),
            components: vec![
                ComponentDecl {
                    name: "worker".into(),
                    image: Some("w".into()),
                    ..ComponentDecl::default()
                },
                ComponentDecl {
                    name: "queue".into(),
                    image: Some("q".into()),
                    ..ComponentDecl::default()
                },
            ],
            connections: vec![ConnectionDecl {
                from: "worker".into(),
                to: "queue".into(),
            }],
        };

        let resolved = resolve_connections(&file).expect("should resolve");
        let worker = resolved
            .iter()
            .find(|r| r.name == "worker")
            .expect("worker");
        assert!(
            worker
                .env
                .iter()
                .any(|(k, v)| k == "QUEUE_HOST" && v == "queue")
        );
        assert!(!worker.env.iter().any(|(k, _)| k == "QUEUE_PORT"));
    }

    #[test]
    fn resolve_multiple_connections() {
        let file = CompositionFile {
            imports: Vec::new(),
            components: vec![
                ComponentDecl {
                    name: "api".into(),
                    image: Some("api".into()),
                    ..ComponentDecl::default()
                },
                ComponentDecl {
                    name: "db".into(),
                    image: Some("db".into()),
                    port: Some(5432),
                    ..ComponentDecl::default()
                },
                ComponentDecl {
                    name: "cache".into(),
                    image: Some("redis".into()),
                    port: Some(6379),
                    ..ComponentDecl::default()
                },
            ],
            connections: vec![
                ConnectionDecl {
                    from: "api".into(),
                    to: "db".into(),
                },
                ConnectionDecl {
                    from: "api".into(),
                    to: "cache".into(),
                },
            ],
        };

        let resolved = resolve_connections(&file).expect("should resolve");
        let api = resolved.iter().find(|r| r.name == "api").expect("api");
        assert!(api.env.iter().any(|(k, _)| k == "DB_HOST"));
        assert!(api.env.iter().any(|(k, _)| k == "DB_PORT"));
        assert!(api.env.iter().any(|(k, _)| k == "CACHE_HOST"));
        assert!(api.env.iter().any(|(k, _)| k == "CACHE_PORT"));
    }

    #[test]
    fn resolve_undefined_target_returns_error() {
        let file = CompositionFile {
            imports: Vec::new(),
            components: vec![ComponentDecl {
                name: "api".into(),
                image: Some("api".into()),
                ..ComponentDecl::default()
            }],
            connections: vec![ConnectionDecl {
                from: "api".into(),
                to: "missing".into(),
            }],
        };

        let result = resolve_connections(&file);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing"));
    }

    #[test]
    fn resolve_undefined_source_returns_error() {
        let file = CompositionFile {
            imports: Vec::new(),
            components: vec![ComponentDecl {
                name: "db".into(),
                image: Some("db".into()),
                ..ComponentDecl::default()
            }],
            connections: vec![ConnectionDecl {
                from: "missing".into(),
                to: "db".into(),
            }],
        };

        let result = resolve_connections(&file);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing"));
    }

    #[test]
    fn resolve_large_connection_chain_remains_linear() {
        let component_count = 2_000;
        let components = (0..component_count)
            .map(|index| ComponentDecl {
                name: format!("service_{index}"),
                image: Some("file:///image".into()),
                port: Some(8_000),
                ..ComponentDecl::default()
            })
            .collect();
        let connections = (1..component_count)
            .map(|index| ConnectionDecl {
                from: format!("service_{index}"),
                to: format!("service_{}", index - 1),
            })
            .collect();
        let file = CompositionFile {
            imports: Vec::new(),
            components,
            connections,
        };

        let resolved = resolve_connections(&file).expect("resolve large graph");
        assert_eq!(resolved.len(), component_count);
        assert_eq!(resolved[1].env.len(), 2);
        assert_eq!(resolved[component_count - 1].env.len(), 2);
    }
}
