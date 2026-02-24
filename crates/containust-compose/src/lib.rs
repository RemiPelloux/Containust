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

pub mod component;
pub mod distroless;
pub mod graph;
pub mod import;
pub mod parser;
pub mod resolver;
