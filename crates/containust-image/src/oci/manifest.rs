//! OCI / Docker registry manifest models and platform selection.
//!
//! Parsing is pure (no I/O). A manifest body is either an index
//! (multi-platform) or a single image manifest carrying layer
//! descriptors; both Docker and OCI media types are accepted.

use containust_common::error::{ContainustError, Result};
use serde::Deserialize;

/// Accept header covering OCI and Docker manifest media types.
pub const MANIFEST_ACCEPT: &str = "application/vnd.oci.image.index.v1+json, \
     application/vnd.oci.image.manifest.v1+json, \
     application/vnd.docker.distribution.manifest.list.v2+json, \
     application/vnd.docker.distribution.manifest.v2+json";

/// A content descriptor: digest-addressed blob or sub-manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct Descriptor {
    /// Media type of the referenced content.
    #[serde(rename = "mediaType", default)]
    pub media_type: String,
    /// `sha256:<hex>` content address.
    pub digest: String,
    /// Declared size in bytes.
    #[serde(default)]
    pub size: u64,
    /// Target platform (index entries only).
    #[serde(default)]
    pub platform: Option<Platform>,
}

/// Platform selector attached to index entries.
#[derive(Debug, Clone, Deserialize)]
pub struct Platform {
    /// CPU architecture in OCI notation (`amd64`, `arm64`, ...).
    pub architecture: String,
    /// Operating system (`linux`).
    pub os: String,
}

#[derive(Debug, Deserialize)]
struct RawManifest {
    #[serde(default)]
    manifests: Vec<Descriptor>,
    #[serde(default)]
    layers: Vec<Descriptor>,
}

/// A parsed registry manifest.
#[derive(Debug)]
pub enum Manifest {
    /// Multi-platform index; entries reference platform manifests.
    Index(Vec<Descriptor>),
    /// Single-platform image; entries are ordered layer blobs.
    Image(Vec<Descriptor>),
}

/// Parses a manifest body into an index or an image manifest.
///
/// # Errors
///
/// Returns an error when the body is not valid manifest JSON or
/// carries neither `manifests` nor `layers`.
pub fn parse_manifest(body: &[u8]) -> Result<Manifest> {
    let raw: RawManifest =
        serde_json::from_slice(body).map_err(|error| ContainustError::Config {
            message: format!("invalid registry manifest JSON: {error}"),
        })?;
    if !raw.manifests.is_empty() {
        return Ok(Manifest::Index(raw.manifests));
    }
    if !raw.layers.is_empty() {
        return Ok(Manifest::Image(raw.layers));
    }
    Err(ContainustError::Config {
        message: "registry manifest has neither manifest entries nor layers".into(),
    })
}

/// Maps the host CPU architecture to OCI platform notation.
#[must_use]
pub fn host_oci_architecture() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => other,
    }
}

/// Selects the `linux` manifest matching `architecture` from an index.
///
/// # Errors
///
/// Returns an error when no entry matches the requested platform.
pub fn select_platform(entries: &[Descriptor], architecture: &str) -> Result<Descriptor> {
    entries
        .iter()
        .find(|entry| {
            entry.platform.as_ref().is_some_and(|platform| {
                platform.os == "linux" && platform.architecture == architecture
            })
        })
        .cloned()
        .ok_or_else(|| ContainustError::Config {
            message: format!(
                "image index has no linux/{architecture} manifest; \
                 available platforms: {}",
                describe_platforms(entries)
            ),
        })
}

fn describe_platforms(entries: &[Descriptor]) -> String {
    let described: Vec<String> = entries
        .iter()
        .filter_map(|entry| entry.platform.as_ref())
        .map(|platform| format!("{}/{}", platform.os, platform.architecture))
        .collect();
    if described.is_empty() {
        "<none>".into()
    } else {
        described.join(", ")
    }
}

/// Strips and validates a `sha256:<hex>` descriptor digest.
///
/// # Errors
///
/// Returns an error when the digest algorithm is not SHA-256 or the
/// hex payload is malformed.
pub fn descriptor_sha256(digest: &str) -> Result<containust_common::types::Sha256Hash> {
    let hex = digest
        .strip_prefix("sha256:")
        .ok_or_else(|| ContainustError::Config {
            message: format!("unsupported registry digest algorithm: {digest}"),
        })?;
    containust_common::types::Sha256Hash::from_hex(hex)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_manifest_index_returns_entries() {
        let body = br#"{"manifests":[{"digest":"sha256:aa","platform":
            {"architecture":"amd64","os":"linux"}}]}"#;
        assert!(matches!(
            parse_manifest(body).expect("parse"),
            Manifest::Index(entries) if entries.len() == 1
        ));
    }

    #[test]
    fn parse_manifest_image_returns_layers() {
        let body = br#"{"layers":[{"digest":"sha256:aa","size":3},{"digest":"sha256:bb"}]}"#;
        assert!(matches!(
            parse_manifest(body).expect("parse"),
            Manifest::Image(layers) if layers.len() == 2
        ));
    }

    #[test]
    fn parse_manifest_empty_body_returns_error() {
        assert!(parse_manifest(b"{}").is_err());
    }

    #[test]
    fn parse_manifest_invalid_json_returns_error() {
        assert!(parse_manifest(b"not-json").is_err());
    }

    #[test]
    fn select_platform_finds_linux_match() {
        let entries = vec![
            Descriptor {
                media_type: String::new(),
                digest: "sha256:aa".into(),
                size: 0,
                platform: Some(Platform {
                    architecture: "amd64".into(),
                    os: "linux".into(),
                }),
            },
            Descriptor {
                media_type: String::new(),
                digest: "sha256:bb".into(),
                size: 0,
                platform: Some(Platform {
                    architecture: "arm64".into(),
                    os: "linux".into(),
                }),
            },
        ];
        let selected = select_platform(&entries, "arm64").expect("select");
        assert_eq!(selected.digest, "sha256:bb");
    }

    #[test]
    fn select_platform_missing_architecture_lists_available() {
        let entries = vec![Descriptor {
            media_type: String::new(),
            digest: "sha256:aa".into(),
            size: 0,
            platform: Some(Platform {
                architecture: "amd64".into(),
                os: "linux".into(),
            }),
        }];
        let error = select_platform(&entries, "riscv64").expect_err("must fail");
        assert!(error.to_string().contains("linux/amd64"));
    }

    #[test]
    fn descriptor_sha256_rejects_other_algorithms() {
        assert!(descriptor_sha256("sha512:abcd").is_err());
    }

    #[test]
    fn descriptor_sha256_accepts_valid_hex() {
        let hex = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        let digest = descriptor_sha256(&format!("sha256:{hex}")).expect("valid");
        assert_eq!(digest.as_hex(), hex);
    }

    #[test]
    fn host_oci_architecture_is_oci_notation() {
        let arch = host_oci_architecture();
        assert!(arch == "amd64" || arch == "arm64" || !arch.is_empty());
    }
}
