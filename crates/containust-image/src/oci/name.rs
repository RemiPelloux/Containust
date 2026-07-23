//! OCI image name parsing (`[registry/]repository[:tag]`).
//!
//! Pure string parsing, no I/O. Docker Hub short names gain the
//! `library/` prefix exactly as the Docker CLI resolves them.

use containust_common::error::{ContainustError, Result};

/// Docker Hub's pull endpoint.
pub const DEFAULT_REGISTRY: &str = "registry-1.docker.io";

/// A fully resolved registry image name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OciName {
    /// Registry host (and optional port).
    pub registry: String,
    /// Repository path, e.g. `library/alpine` or `org/app`.
    pub repository: String,
    /// Tag; defaults to `latest`.
    pub tag: String,
}

/// Parses the location part of an `oci://` reference.
///
/// # Errors
///
/// Returns an error when the repository or tag is empty.
pub fn parse_oci_name(location: &str) -> Result<OciName> {
    let invalid = |detail: &str| ContainustError::Config {
        message: format!("invalid oci:// image name '{location}': {detail}"),
    };
    let (registry, remainder) = split_registry(location);
    let (repository, tag) = split_tag(remainder);
    if repository.is_empty() {
        return Err(invalid("repository is empty"));
    }
    if tag.is_empty() {
        return Err(invalid("tag is empty"));
    }
    let repository = qualify_repository(&registry, repository);
    Ok(OciName {
        registry,
        repository,
        tag: tag.to_string(),
    })
}

/// Splits a leading registry host from the repository path.
///
/// The first segment is a registry when it contains a dot or a port,
/// or is `localhost` — the same heuristic the Docker CLI uses.
fn split_registry(location: &str) -> (String, &str) {
    match location.split_once('/') {
        Some((first, rest))
            if first.contains('.') || first.contains(':') || first == "localhost" =>
        {
            (first.to_string(), rest)
        }
        _ => (DEFAULT_REGISTRY.to_string(), location),
    }
}

/// Splits an optional `:tag` suffix from the repository path.
fn split_tag(remainder: &str) -> (&str, &str) {
    // Only the final path segment may carry a tag separator.
    match remainder.rsplit_once(':') {
        Some((repository, tag)) if !tag.contains('/') => (repository, tag),
        _ => (remainder, "latest"),
    }
}

/// Docker Hub short names resolve under the `library/` namespace.
fn qualify_repository(registry: &str, repository: &str) -> String {
    if registry == DEFAULT_REGISTRY && !repository.contains('/') {
        format!("library/{repository}")
    } else {
        repository.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_short_name_defaults_to_docker_hub_library() {
        let name = parse_oci_name("alpine").expect("parse");
        assert_eq!(name.registry, DEFAULT_REGISTRY);
        assert_eq!(name.repository, "library/alpine");
        assert_eq!(name.tag, "latest");
    }

    #[test]
    fn parse_short_name_with_tag_keeps_tag() {
        let name = parse_oci_name("alpine:3.21").expect("parse");
        assert_eq!(name.repository, "library/alpine");
        assert_eq!(name.tag, "3.21");
    }

    #[test]
    fn parse_namespaced_hub_name_is_not_library_qualified() {
        let name = parse_oci_name("grafana/grafana:11.0.0").expect("parse");
        assert_eq!(name.registry, DEFAULT_REGISTRY);
        assert_eq!(name.repository, "grafana/grafana");
    }

    #[test]
    fn parse_ghcr_name_extracts_registry() {
        let name = parse_oci_name("ghcr.io/org/app:v1").expect("parse");
        assert_eq!(name.registry, "ghcr.io");
        assert_eq!(name.repository, "org/app");
        assert_eq!(name.tag, "v1");
    }

    #[test]
    fn parse_localhost_registry_with_port() {
        let name = parse_oci_name("localhost:5000/app:dev").expect("parse");
        assert_eq!(name.registry, "localhost:5000");
        assert_eq!(name.repository, "app");
        assert_eq!(name.tag, "dev");
    }

    #[test]
    fn parse_empty_repository_returns_error() {
        assert!(parse_oci_name("ghcr.io/:v1").is_err());
    }

    #[test]
    fn parse_empty_tag_returns_error() {
        assert!(parse_oci_name("alpine:").is_err());
    }
}
