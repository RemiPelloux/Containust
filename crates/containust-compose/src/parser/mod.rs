//! `.ctst` file parser built on `nom`.
//!
//! Transforms raw `.ctst` text into a validated AST through
//! lexing, parsing, and static analysis phases.

pub mod ast;
pub mod lexer;
pub mod validator;

use containust_common::error::Result;

use self::ast::CompositionFile;

/// Parses a `.ctst` file from its source text.
///
/// # Errors
///
/// Returns an error if the input contains syntax errors or fails validation.
pub fn parse_ctst(_input: &str) -> Result<CompositionFile> {
    tracing::info!("parsing .ctst input");
    todo!()
}
