//! Integration tests for .ctst composition and dependency graph resolution.
//!
//! These tests are implemented in:
//! `crates/containust-runtime/tests/e2e_test.rs`
//!
//! Covered scenarios:
//! - `pipeline_dependency_order_respects_connections`: Verifies db/cache deploy before api
//! - `pipeline_cycle_detection`: Circular CONNECT declarations produce an error
//! - `pipeline_connection_env_injection`: CONNECT generates DATABASE_HOST/DATABASE_PORT
//! - `pipeline_resolver_preserves_component_env`: Original env vars are preserved after resolution
//! - `pipeline_parse_multi_component_with_connections`: Multi-component parsing with connections
