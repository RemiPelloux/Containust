//! # containust-common
//!
//! Shared types, error definitions, configuration models, and constants
//! used across the entire Containust workspace.
//!
//! This crate is the leaf of the dependency graph — it depends on no other
//! internal crate and provides the foundational primitives that all other
//! crates build upon.

#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used, unsafe_code))]

pub mod config;
pub mod constants;
pub mod error;
pub mod redact;
pub mod types;
