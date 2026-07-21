//! # containust-image
//!
//! Container image and layer management for the Containust runtime.
//!
//! Handles:
//! - **References**: structured `file://`, `tar://`, `image://`, and remote URIs.
//! - **Import**: deterministic content-addressed import and materialization.
//! - **Fetch**: explicit opt-in remote downloads with offline enforcement.
//! - **Layers**: diff-based filesystem layers with caching.
//! - **Storage**: local content-addressed storage for images and layers.
//! - **Hashing**: SHA-256 content verification.
//! - **FUSE**: lazy-loading for fast container startup.
//! - **Registry**: local image catalog management.

#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

pub mod fetch;
pub mod fuse;
pub mod hash;
pub mod import;
pub mod layer;
pub mod pack;
pub mod reference;
pub mod registry;
pub mod source;
pub mod storage;
