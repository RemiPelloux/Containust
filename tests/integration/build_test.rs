//! Integration tests for image build pipeline.
//!
//! These tests are implemented in:
//! `crates/containust-runtime/tests/e2e_test.rs`
//!
//! Covered scenarios:
//! - `pipeline_parse_hello_world_ctst`: Parses a simple .ctst and verifies all fields
//! - `pipeline_validator_rejects_missing_image`: Proper error on missing image
//! - `pipeline_validator_rejects_duplicate_names`: Proper error on duplicate names
//! - `pipeline_validator_rejects_undefined_connection`: Error on malformed CONNECT
//! - `pipeline_layer_extraction`: Extract tar layers and verify contents
//! - `pipeline_sha256_hashing`: SHA-256 content verification
