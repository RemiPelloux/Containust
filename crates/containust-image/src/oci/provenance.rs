//! Optional OCI image provenance / signature checks (P11.9).
//!
//! When enabled, pulls fail closed unless `cosign` can verify a signature
//! for the resolved manifest digest. Identity/issuer regexps are taken from
//! the environment (or permissive defaults that still require *a* signature).

use std::process::Command;

use containust_common::error::{ContainustError, Result};
use containust_common::types::Sha256Hash;

use crate::oci::name::OciName;

/// Policy for optional provenance enforcement on registry pulls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProvenancePolicy {
    /// When true, require a successful `cosign verify` before accepting layers.
    pub require: bool,
}

/// Verifies image provenance when [`ProvenancePolicy::require`] is set.
///
/// # Errors
///
/// Returns an error when provenance is required and `cosign` is missing,
/// exits non-zero, or cannot be spawned.
pub fn ensure_image_provenance(
    name: &OciName,
    manifest_digest: &Sha256Hash,
    policy: ProvenancePolicy,
) -> Result<()> {
    if !policy.require {
        return Ok(());
    }
    let image = format!(
        "{}/{}@sha256:{}",
        name.registry,
        name.repository,
        manifest_digest.as_hex()
    );
    verify_with_cosign(&image)
}

fn verify_with_cosign(image: &str) -> Result<()> {
    let identity =
        std::env::var("CONTAINUST_COSIGN_IDENTITY_REGEXP").unwrap_or_else(|_| ".*".into());
    let issuer =
        std::env::var("CONTAINUST_COSIGN_OIDC_ISSUER_REGEXP").unwrap_or_else(|_| ".*".into());

    let output = Command::new("cosign")
        .args([
            "verify",
            "--certificate-identity-regexp",
            &identity,
            "--certificate-oidc-issuer-regexp",
            &issuer,
            image,
        ])
        .output()
        .map_err(|source| ContainustError::Config {
            message: format!(
                "provenance required but cosign could not be executed ({source}); \
                 install cosign or omit --require-provenance"
            ),
        })?;

    if output.status.success() {
        tracing::info!(%image, "cosign provenance verified");
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(ContainustError::Config {
        message: format!(
            "provenance required but cosign verify failed for {image}: {}",
            stderr.trim()
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use containust_common::types::Sha256Hash;

    #[test]
    fn ensure_provenance_noop_when_not_required() {
        let name = crate::oci::name::parse_oci_name("alpine:3.21").expect("name");
        let digest = Sha256Hash::from_hex(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        )
        .expect("digest");
        ensure_image_provenance(&name, &digest, ProvenancePolicy::default()).expect("noop");
    }
}
