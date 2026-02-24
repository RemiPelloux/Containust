//! Integration tests for container runtime operations.
//!
//! These tests are implemented in:
//! `crates/containust-runtime/tests/e2e_test.rs`
//!
//! Covered scenarios:
//! - `pipeline_state_persistence_roundtrip`: Save/load state with containers
//! - `pipeline_state_all_lifecycle_states`: All 4 lifecycle states survive serialization
//! - `pipeline_log_append_and_read`: Container log append and read
//! - `pipeline_log_isolation_between_containers`: Logs are isolated per container
//! - `pipeline_platform_detection`: Platform info returns valid OS/arch
//! - `pipeline_image_catalog_crud`: Image catalog create/read/delete operations
