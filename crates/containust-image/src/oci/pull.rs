//! Registry pull flow: token exchange, manifest resolution, layer blobs.
//!
//! Every downloaded byte is verified: the manifest body hash must match
//! the pinned digest (when given) and each layer blob must match the
//! digest declared by its manifest descriptor. Mismatches delete the
//! staged file and fail closed.

use std::io::Read;

use containust_common::error::{ContainustError, Result};
use containust_common::types::Sha256Hash;
use sha2::{Digest, Sha256};

use crate::fetch::{FetchPolicy, build_client, copy_capped};
use crate::oci::auth;
use crate::oci::manifest::{
    Descriptor, MANIFEST_ACCEPT, Manifest, descriptor_sha256, host_oci_architecture,
    parse_manifest, select_platform,
};
use crate::oci::name::{OciName, parse_oci_name};
use crate::oci::provenance::{ProvenancePolicy, ensure_image_provenance};
use crate::reference::ImageReference;
use crate::storage::StorageBackend;

/// Manifest documents are small; anything larger is suspect.
const MANIFEST_MAX_BYTES: u64 = 4 * 1024 * 1024;

/// A verified layer blob staged on disk, in manifest order.
#[derive(Debug)]
pub struct LayerBlob {
    /// Staged blob file awaiting commit into the layer store.
    pub path: std::path::PathBuf,
    /// Verified SHA-256 of the blob content.
    pub digest: Sha256Hash,
    /// Blob size in bytes.
    pub size: u64,
}

/// The result of a completed registry pull.
#[derive(Debug)]
pub struct PulledImage {
    /// SHA-256 of the top-level manifest document (index or image).
    pub manifest_digest: Sha256Hash,
    /// Verified layer blobs in extraction order.
    pub layers: Vec<LayerBlob>,
}

/// Pulls an `oci://` reference into staged, digest-verified layer blobs.
///
/// # Errors
///
/// Returns an error when offline mode is enabled, the name is invalid,
/// authentication fails, no linux manifest matches the host platform,
/// provenance is required and fails, or any downloaded content fails
/// digest verification.
pub fn pull_image(
    store: &StorageBackend,
    reference: &ImageReference,
    policy: &FetchPolicy,
    provenance: ProvenancePolicy,
) -> Result<PulledImage> {
    if policy.offline {
        return Err(ContainustError::Network {
            url: reference.canonical_uri(),
            message: "offline mode blocks registry pulls; pull on a connected machine \
                      and copy the layer store"
                .into(),
        });
    }
    let name = parse_oci_name(reference.location())?;
    let session = RegistrySession::open(&name, policy)?;

    let manifest_part = reference.digest().map_or_else(
        || name.tag.clone(),
        |pin| format!("sha256:{}", pin.as_hex()),
    );
    let (body, manifest_digest) = session.fetch_manifest(&manifest_part)?;
    verify_pin(reference, &manifest_digest)?;
    ensure_image_provenance(&name, &manifest_digest, provenance)?;

    let layers = session.resolve_layer_descriptors(&body)?;
    let layers = layers
        .iter()
        .map(|descriptor| session.download_layer(store, descriptor))
        .collect::<Result<Vec<_>>>()?;
    tracing::info!(
        repository = %name.repository,
        digest = %manifest_digest,
        layer_count = layers.len(),
        "registry image pulled and verified"
    );
    Ok(PulledImage {
        manifest_digest,
        layers,
    })
}

fn verify_pin(reference: &ImageReference, actual: &Sha256Hash) -> Result<()> {
    let Some(pinned) = reference.digest() else {
        return Ok(());
    };
    if pinned.as_hex() == actual.as_hex() {
        return Ok(());
    }
    Err(ContainustError::HashMismatch {
        resource: reference.to_string(),
        expected: pinned.as_hex().to_string(),
        actual: actual.as_hex().to_string(),
    })
}

/// One authenticated conversation with a single registry repository.
struct RegistrySession {
    client: reqwest::blocking::Client,
    base: String,
    repository: String,
    token: Option<String>,
    max_blob_bytes: u64,
}

impl RegistrySession {
    /// Builds a client and resolves a pull token for the repository.
    fn open(name: &OciName, policy: &FetchPolicy) -> Result<Self> {
        let base = format!("https://{}", name.registry);
        let client = build_client(policy).map_err(|error| ContainustError::Network {
            url: base.clone(),
            message: format!("failed to construct HTTP client: {error}"),
        })?;
        let mut session = Self {
            client,
            base,
            repository: name.repository.clone(),
            token: auth::env_bearer_token(),
            max_blob_bytes: policy.max_bytes,
        };
        if session.token.is_none() {
            session.token = session.negotiate_token(name)?;
        }
        Ok(session)
    }

    /// Probes `/v2/` and performs the bearer challenge dance if needed.
    fn negotiate_token(&self, name: &OciName) -> Result<Option<String>> {
        let url = format!("{}/v2/", self.base);
        let response = self
            .client
            .get(&url)
            .send()
            .map_err(|error| network_error(&url, format!("probe failed: {error}")))?;
        if response.status() != reqwest::StatusCode::UNAUTHORIZED {
            return Ok(None);
        }
        let challenge = response
            .headers()
            .get(reqwest::header::WWW_AUTHENTICATE)
            .and_then(|value| value.to_str().ok())
            .and_then(auth::parse_bearer_challenge)
            .ok_or_else(|| {
                network_error(&url, "registry requires unsupported authentication".into())
            })?;
        let credentials = auth::basic_credentials(&name.registry);
        let token = auth::fetch_bearer_token(
            &self.client,
            &challenge,
            &self.repository,
            credentials.as_ref(),
        )?;
        Ok(Some(token))
    }

    fn get(&self, url: &str, accept: &str) -> Result<reqwest::blocking::Response> {
        let mut request = self.client.get(url).header(reqwest::header::ACCEPT, accept);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .map_err(|error| network_error(url, format!("request failed: {error}")))?;
        let status = response.status();
        if !status.is_success() {
            return Err(network_error(
                url,
                format!("registry returned status {status}; check the image name and credentials"),
            ));
        }
        Ok(response)
    }

    /// Fetches a manifest by tag or digest, hashing the body in one pass.
    fn fetch_manifest(&self, manifest_part: &str) -> Result<(Vec<u8>, Sha256Hash)> {
        let url = format!(
            "{}/v2/{}/manifests/{manifest_part}",
            self.base, self.repository
        );
        let response = self.get(&url, MANIFEST_ACCEPT)?;
        let mut body = Vec::new();
        let read = response
            .take(MANIFEST_MAX_BYTES.saturating_add(1))
            .read_to_end(&mut body)
            .map_err(|error| network_error(&url, format!("stream interrupted: {error}")))?;
        if read as u64 > MANIFEST_MAX_BYTES {
            return Err(network_error(
                &url,
                format!("manifest exceeds the {MANIFEST_MAX_BYTES} byte limit"),
            ));
        }
        let digest = Sha256::digest(&body);
        let digest = Sha256Hash::from_hex(format!("{digest:x}"))?;
        Ok((body, digest))
    }

    /// Resolves the ordered layer descriptors, descending through a
    /// platform index when necessary.
    fn resolve_layer_descriptors(&self, body: &[u8]) -> Result<Vec<Descriptor>> {
        match parse_manifest(body)? {
            Manifest::Image(layers) => Ok(layers),
            Manifest::Index(entries) => {
                let selected = select_platform(&entries, host_oci_architecture())?;
                let expected = descriptor_sha256(&selected.digest)?;
                let (sub_body, sub_digest) = self.fetch_manifest(&selected.digest)?;
                if sub_digest.as_hex() != expected.as_hex() {
                    return Err(ContainustError::HashMismatch {
                        resource: selected.digest,
                        expected: expected.as_hex().to_string(),
                        actual: sub_digest.as_hex().to_string(),
                    });
                }
                match parse_manifest(&sub_body)? {
                    Manifest::Image(layers) => Ok(layers),
                    Manifest::Index(_) => Err(ContainustError::Config {
                        message: "registry returned a nested image index; \
                                  nested indexes are not supported"
                            .into(),
                    }),
                }
            }
        }
    }

    /// Downloads one layer blob to a staging path and verifies it.
    fn download_layer(&self, store: &StorageBackend, descriptor: &Descriptor) -> Result<LayerBlob> {
        let expected = descriptor_sha256(&descriptor.digest)?;
        let url = format!(
            "{}/v2/{}/blobs/{}",
            self.base, self.repository, descriptor.digest
        );
        let response = self.get(&url, "application/octet-stream")?;
        let staged = store.staging_path();
        let actual = copy_capped(response, &staged, self.max_blob_bytes, &url)?;
        if actual.as_hex() != expected.as_hex() {
            let _ = std::fs::remove_file(&staged);
            return Err(ContainustError::HashMismatch {
                resource: url,
                expected: expected.as_hex().to_string(),
                actual: actual.as_hex().to_string(),
            });
        }
        let size = std::fs::metadata(&staged)
            .map_err(|source| ContainustError::Io {
                path: staged.clone(),
                source,
            })?
            .len();
        Ok(LayerBlob {
            path: staged,
            digest: actual,
            size,
        })
    }
}

fn network_error(url: &str, message: String) -> ContainustError {
    ContainustError::Network {
        url: url.to_string(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pull_offline_policy_rejects_before_connecting() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = StorageBackend::open(dir.path().to_path_buf()).expect("open store");
        let reference = ImageReference::parse("oci://alpine:3.21").expect("parse");
        let policy = FetchPolicy {
            offline: true,
            ..FetchPolicy::default()
        };
        let error = pull_image(&store, &reference, &policy, ProvenancePolicy::default())
            .expect_err("offline must fail");
        assert!(error.to_string().contains("offline"));
    }

    #[test]
    fn verify_pin_accepts_matching_digest() {
        let hex = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        let reference =
            ImageReference::parse(&format!("oci://alpine:3.21@sha256:{hex}")).expect("parse");
        let digest = Sha256Hash::from_hex(hex).expect("hash");
        assert!(verify_pin(&reference, &digest).is_ok());
    }

    #[test]
    fn verify_pin_rejects_mismatched_digest() {
        let pinned = "0".repeat(64);
        let reference =
            ImageReference::parse(&format!("oci://alpine:3.21@sha256:{pinned}")).expect("parse");
        let actual = Sha256Hash::from_hex(
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
        )
        .expect("hash");
        let error = verify_pin(&reference, &actual).expect_err("mismatch must fail");
        assert!(matches!(error, ContainustError::HashMismatch { .. }));
    }
}
