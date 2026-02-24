//! Dependency graph management using `petgraph`.
//!
//! Builds a directed acyclic graph from component connections
//! and resolves topological ordering for deployment.

use containust_common::error::{ContainustError, Result};

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

    /// Adds a dependency edge: `dependent` depends on `dependency`.
    ///
    /// The graph edge points from `dependency` to `dependent`
    /// so that topological sort yields dependencies first.
    pub fn add_dependency(
        &mut self,
        dependent: petgraph::graph::NodeIndex,
        dependency: petgraph::graph::NodeIndex,
    ) {
        let _ = self.graph.add_edge(dependency, dependent, ());
    }

    /// Returns a topological ordering of components for deployment.
    ///
    /// Dependencies appear before the components that depend on them
    /// (the order is reversed from `petgraph::algo::toposort`).
    ///
    /// # Errors
    ///
    /// Returns an error if the graph contains cycles.
    pub fn resolve_order(&self) -> Result<Vec<String>> {
        match petgraph::algo::toposort(&self.graph, None) {
            Ok(indices) => {
                let names: Vec<String> = indices
                    .iter()
                    .filter_map(|&idx| self.graph.node_weight(idx).cloned())
                    .collect();
                Ok(names)
            }
            Err(_cycle) => Err(ContainustError::Config {
                message: "cyclic dependency detected in component graph".into(),
            }),
        }
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph_resolves_to_empty() {
        let graph = DependencyGraph::new();
        let order = graph.resolve_order().expect("should resolve");
        assert!(order.is_empty());
    }

    #[test]
    fn single_node_resolves() {
        let mut graph = DependencyGraph::new();
        let _ = graph.add_component("api");
        let order = graph.resolve_order().expect("should resolve");
        assert_eq!(order, vec!["api"]);
    }

    #[test]
    fn linear_dependency_chain() {
        let mut graph = DependencyGraph::new();
        let api = graph.add_component("api");
        let db = graph.add_component("db");
        graph.add_dependency(api, db);

        let order = graph.resolve_order().expect("should resolve");
        let api_pos = order.iter().position(|n| n == "api").expect("api");
        let db_pos = order.iter().position(|n| n == "db").expect("db");
        assert!(db_pos < api_pos, "db should come before api: {order:?}");
    }

    #[test]
    fn diamond_dependency() {
        let mut graph = DependencyGraph::new();
        let a = graph.add_component("a");
        let b = graph.add_component("b");
        let c = graph.add_component("c");
        let d = graph.add_component("d");
        graph.add_dependency(a, b);
        graph.add_dependency(a, c);
        graph.add_dependency(b, d);
        graph.add_dependency(c, d);

        let order = graph.resolve_order().expect("should resolve");
        assert_eq!(order.len(), 4);
        let pos = |name: &str| order.iter().position(|n| n == name).expect(name);
        assert!(pos("d") < pos("b"));
        assert!(pos("d") < pos("c"));
        assert!(pos("b") < pos("a"));
        assert!(pos("c") < pos("a"));
    }

    #[test]
    fn cycle_detection() {
        let mut graph = DependencyGraph::new();
        let a = graph.add_component("a");
        let b = graph.add_component("b");
        graph.add_dependency(a, b);
        graph.add_dependency(b, a);

        let result = graph.resolve_order();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("cyclic"), "got: {msg}");
    }

    #[test]
    fn three_node_cycle_detection() {
        let mut graph = DependencyGraph::new();
        let a = graph.add_component("a");
        let b = graph.add_component("b");
        let c = graph.add_component("c");
        graph.add_dependency(a, b);
        graph.add_dependency(b, c);
        graph.add_dependency(c, a);

        let result = graph.resolve_order();
        assert!(result.is_err());
    }

    #[test]
    fn independent_nodes_all_present() {
        let mut graph = DependencyGraph::new();
        let _ = graph.add_component("x");
        let _ = graph.add_component("y");
        let _ = graph.add_component("z");

        let order = graph.resolve_order().expect("should resolve");
        assert_eq!(order.len(), 3);
        assert!(order.contains(&"x".to_string()));
        assert!(order.contains(&"y".to_string()));
        assert!(order.contains(&"z".to_string()));
    }
}
