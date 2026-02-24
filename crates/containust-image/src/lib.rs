//! # containust-image
//!
//! Container image and layer management for the Containust runtime.
//!
//! Handles:
//! - **Layers**: Diff-based filesystem layers with caching.
//! - **Storage**: Local storage backend for images and layers.
//! - **Sources**: `file://`, `tar://`, and remote source protocols.
//! - **Hashing**: SHA-256 content verification.
//! - **FUSE**: Lazy-loading for fast container startup.
//! - **Registry**: Local image catalog management.

#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

pub mod fuse;
pub mod hash;
pub mod layer;
pub mod registry;
pub mod source;
pub mod storage;
