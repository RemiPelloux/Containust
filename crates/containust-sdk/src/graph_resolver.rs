//! Validates and resolves component dependency graphs.
//!
//! Wraps `containust-compose`'s graph and resolver modules into
//! a high-level API for SDK consumers.

use containust_common::error::Result;

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
    pub fn load_ctst(&mut self, _path: &std::path::Path) -> Result<()> {
        tracing::info!(path = %_path.display(), "loading .ctst file");
        todo!()
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
