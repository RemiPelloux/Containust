//! Auto-wiring and environment variable injection.
//!
//! Automatically generates connection environment variables when
//! components are linked via `CONNECT` declarations.

use containust_common::error::Result;

use crate::parser::ast::CompositionFile;

/// Resolves connections and generates environment variables for each component.
///
/// # Errors
///
/// Returns an error if a connection references an undefined component.
pub fn resolve_connections(_file: &CompositionFile) -> Result<Vec<ResolvedComponent>> {
    tracing::info!("resolving connections and injecting environment variables");
    todo!()
}

/// A component with its resolved environment variables.
#[derive(Debug, Clone)]
pub struct ResolvedComponent {
    /// Component name.
    pub name: String,
    /// Environment variables including auto-wired connection vars.
    pub env: Vec<(String, String)>,
}
