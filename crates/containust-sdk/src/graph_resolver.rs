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
