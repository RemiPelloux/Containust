//! Dependency graph management using `petgraph`.
//!
//! Builds a directed acyclic graph from component connections
//! and resolves topological ordering for deployment.

use containust_common::error::Result;

/// A dependency graph of components.
#[derive(Debug)]
pub struct DependencyGraph {
    /// Internal petgraph representation.
    graph: petgraph::Graph<String, ()>,
}

impl DependencyGraph {
    /// Creates an empty dependency graph.
    #[must_use]
    pub fn new() -> Self {
        Self {
            graph: petgraph::Graph::new(),
        }
    }

    /// Adds a component node to the graph.
    pub fn add_component(&mut self, name: impl Into<String>) -> petgraph::graph::NodeIndex {
        self.graph.add_node(name.into())
    }

    /// Adds a dependency edge from one component to another.
    pub fn add_dependency(
        &mut self,
        from: petgraph::graph::NodeIndex,
        to: petgraph::graph::NodeIndex,
    ) {
        let _ = self.graph.add_edge(from, to, ());
    }

    /// Returns a topological ordering of components for deployment.
    ///
    /// # Errors
    ///
    /// Returns an error if the graph contains cycles.
    pub fn resolve_order(&self) -> Result<Vec<String>> {
        tracing::info!("resolving deployment order");
        todo!()
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}
