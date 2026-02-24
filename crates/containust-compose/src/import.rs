//! IMPORT resolution from files and network sources.
//!
//! Resolves `IMPORT` declarations to their source `.ctst` files,
//! supporting local file paths and (when online) remote URLs.

use std::path::Path;

use containust_common::error::Result;

use crate::parser::ast::CompositionFile;

/// Resolves an import declaration and parses the referenced file.
///
/// # Errors
///
/// Returns an error if the source cannot be found, read, or parsed.
pub fn resolve_import(_source: &str, _base_dir: &Path) -> Result<CompositionFile> {
    tracing::info!(source = _source, "resolving import");
    todo!()
}
