//! Validates and resolves component dependency graphs.
//!
//! Wraps `containust-compose`'s graph and resolver modules into
//! a high-level API for SDK consumers.

use containust_common::error::{ContainustError, Result};

/// High-level resolver for component dependency graphs.
#[derive(Debug)]
pub struct GraphResolver {
    graph: containust_compose::graph::DependencyGraph,
}

impl GraphResolver {
    /// Creates a new empty graph resolver.
    #[must_use]
    pub fn new() -> Self {
        Self {
            graph: containust_compose::graph::DependencyGraph::new(),
        }
    }

    /// Loads and resolves a `.ctst` composition file.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing, validation, or resolution fails.
    pub fn load_ctst(&mut self, path: &std::path::Path) -> Result<()> {
        tracing::info!(path = %path.display(), "loading .ctst file");

        let content = std::fs::read_to_string(path).map_err(|e| ContainustError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;

        let composition = containust_compose::parser::parse_ctst(&content)?;
        containust_compose::parser::validator::validate(&composition)?;

        self.graph = containust_compose::graph::DependencyGraph::new();
        let mut node_map = std::collections::HashMap::new();
        for comp in &composition.components {
            let idx = self.graph.add_component(&comp.name);
            let _ = node_map.insert(comp.name.clone(), idx);
        }
        for conn in &composition.connections {
            if let (Some(&from), Some(&to)) = (node_map.get(&conn.from), node_map.get(&conn.to)) {
                self.graph.add_dependency(from, to);
            }
        }

        Ok(())
    }

    /// Returns the deployment order for all components.
    ///
    /// # Errors
    ///
    /// Returns an error if the graph contains cycles.
    pub fn deployment_order(&self) -> Result<Vec<String>> {
        self.graph.resolve_order()
    }
}

impl Default for GraphResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::io::Write;

    use crate::graph_resolver::GraphResolver;

    #[test]
    fn graph_resolver_new_starts_empty() {
        let resolver = GraphResolver::new();
        // Empty graph resolves to empty order
        assert!(resolver.deployment_order().is_ok());
    }

    #[test]
    fn graph_resolver_default_equals_new() {
        let a = GraphResolver::new();
        let b = GraphResolver::default();
        assert_eq!(
            a.deployment_order().unwrap().len(),
            b.deployment_order().unwrap().len()
        );
    }

    #[test]
    fn graph_resolver_load_ctst_parses_and_populates_graph() {
        let mut content = tempfile::NamedTempFile::new().expect("create temp file");
        content
            .write_all(
                b"COMPONENT api {\n    image = \"file:///opt/api\"\n}\n\
                  COMPONENT db {\n    image = \"file:///opt/db\"\n}\n\
                  CONNECT api -> db\n",
            )
            .expect("write");

        let mut resolver = GraphResolver::new();
        let result = resolver.load_ctst(content.path());
        assert!(result.is_ok(), "should load ctst file: {result:?}");

        let order = resolver.deployment_order().expect("resolve should succeed");
        assert_eq!(order.len(), 2);
        let db_pos = order.iter().position(|n| n == "db").expect("db present");
        let api_pos = order.iter().position(|n| n == "api").expect("api present");
        assert!(db_pos < api_pos, "db must deploy before api");
    }

    #[test]
    fn graph_resolver_load_missing_file_returns_error() {
        let mut resolver = GraphResolver::new();
        let missing = std::path::Path::new("/nonexistent/path/file.ctst");
        assert!(resolver.load_ctst(missing).is_err());
    }

    #[test]
    fn graph_resolver_load_invalid_ctst_returns_error() {
        let mut content = tempfile::NamedTempFile::new().expect("create temp file");
        content
            .write_all(b"COMPONENT bad {\n    # missing image\n}\n")
            .expect("write");

        let mut resolver = GraphResolver::new();
        assert!(resolver.load_ctst(content.path()).is_err());
    }

    #[test]
    fn graph_resolver_complex_dependency_order() {
        let mut content = tempfile::NamedTempFile::new().expect("create temp file");
        content
            .write_all(
                b"COMPONENT frontend {\n    image = \"file:///fe\"\n}\n\
                  COMPONENT api {\n    image = \"file:///api\"\n}\n\
                  COMPONENT db {\n    image = \"file:///db\"\n}\n\
                  COMPONENT cache {\n    image = \"file:///cache\"\n}\n\
                  CONNECT frontend -> api\n\
                  CONNECT api -> db\n\
                  CONNECT api -> cache\n",
            )
            .expect("write");

        let mut resolver = GraphResolver::new();
        resolver.load_ctst(content.path()).expect("load ctst");
        let order = resolver.deployment_order().expect("resolve");
        assert_eq!(order.len(), 4);

        let api_pos = order.iter().position(|n| n == "api").expect("api");
        let db_pos = order.iter().position(|n| n == "db").expect("db");
        let cache_pos = order.iter().position(|n| n == "cache").expect("cache");
        let fe_pos = order
            .iter()
            .position(|n| n == "frontend")
            .expect("frontend");

        assert!(db_pos < api_pos, "db before api");
        assert!(cache_pos < api_pos, "cache before api");
        assert!(api_pos < fe_pos, "api before frontend");
    }

    #[test]
    fn graph_resolver_load_replaces_existing_graph() {
        // Load first file
        let mut first = tempfile::NamedTempFile::new().expect("create");
        first
            .write_all(b"COMPONENT a {\n    image = \"file:///a\"\n}\n")
            .expect("write");
        let mut resolver = GraphResolver::new();
        resolver.load_ctst(first.path()).expect("load first");

        // Load second file — should replace the first
        let mut second = tempfile::NamedTempFile::new().expect("create");
        second
            .write_all(
                b"COMPONENT x {\n    image = \"file:///x\"\n}\n\
                  COMPONENT y {\n    image = \"file:///y\"\n}\n",
            )
            .expect("write");
        resolver.load_ctst(second.path()).expect("load second");

        let order = resolver.deployment_order().expect("resolve");
        assert_eq!(order.len(), 2);
        assert!(order.contains(&"x".to_string()));
        assert!(order.contains(&"y".to_string()));
        assert!(!order.contains(&"a".to_string()));
    }

    #[test]
    fn graph_resolver_debug_output() {
        let resolver = GraphResolver::new();
        let debug = format!("{resolver:?}");
        assert!(debug.contains("GraphResolver"));
    }
}
