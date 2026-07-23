//! OCI registry image pull (`oci://` scheme).
//!
//! Resolves `[registry/]repository[:tag]` names against Docker Hub,
//! GHCR, or any OCI distribution registry, verifies every manifest and
//! layer blob by SHA-256, and stages the layers for the local
//! content-addressed store.

pub mod auth;
pub mod manifest;
pub mod name;
pub mod provenance;
pub mod pull;

pub use name::{DEFAULT_REGISTRY, OciName, parse_oci_name};
pub use provenance::ProvenancePolicy;
pub use pull::{LayerBlob, PulledImage, pull_image};
