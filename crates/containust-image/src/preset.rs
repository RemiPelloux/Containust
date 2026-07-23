//! Curated image presets for well-known minimal root filesystems.
//!
//! Presets map short names such as `preset://alpine` to pinned official
//! downloads (URL + SHA-256 + architecture). They are intentionally
//! small rootfs archives — not Docker Hub multi-layer images — so
//! Containust stays lighter and air-gap friendly.
//!
//! After the first online import, the content-addressed layer store
//! satisfies the same preset offline without contacting the network.

use containust_common::error::{ContainustError, Result};
use containust_common::types::Sha256Hash;

use crate::preset_catalog::{PRESETS, UNSUPPORTED_PRESETS};
use crate::reference::ImageReference;

/// A curated preset available for the current host architecture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePreset {
    /// Canonical name without version (`alpine`, `busybox`).
    pub name: &'static str,
    /// Version tag (`3.22`, `latest`).
    pub version: &'static str,
    /// Short human description.
    pub description: &'static str,
    /// Absolute HTTPS URL of the archive.
    pub url: &'static str,
    /// SHA-256 hex digest of the archive bytes.
    pub sha256: &'static str,
    /// Target CPU architecture (`x86_64`, `aarch64`).
    pub arch: &'static str,
}

/// Resolves a `preset://name[:version]` reference for the host architecture.
///
/// # Errors
///
/// Returns an error when the preset name is unknown, the version is
/// unsupported, or no artifact is published for the host architecture.
pub fn resolve_preset(reference: &ImageReference) -> Result<ImagePreset> {
    if reference.scheme() != crate::reference::ImageScheme::Preset {
        return Err(ContainustError::Config {
            message: format!("expected a preset:// reference, got: {reference}"),
        });
    }
    let (name, version) = split_name_version(reference.location())?;
    let arch = host_arch()?;
    let preset = PRESETS
        .iter()
        .find(|preset| {
            preset.name == name
                && preset.arch == arch
                && (preset.version == version || (version == "latest" && preset.default_latest))
        })
        .ok_or_else(|| unknown_preset(name, version, arch))?;
    Ok(ImagePreset {
        name: preset.name,
        version: preset.version,
        description: preset.description,
        url: preset.url,
        sha256: preset.sha256,
        arch: preset.arch,
    })
}

/// Builds a pinned HTTPS [`ImageReference`] for a resolved preset.
///
/// # Errors
///
/// Returns an error if the curated digest is not a valid SHA-256 hex string.
pub fn preset_fetch_reference(preset: &ImagePreset) -> Result<ImageReference> {
    let digest = Sha256Hash::from_hex(preset.sha256)?;
    ImageReference::parse(&format!("{}@sha256:{}", preset.url, digest.as_hex()))
}

/// Lists every curated preset for the host architecture.
#[must_use]
pub fn list_presets() -> Vec<ImagePreset> {
    let Ok(arch) = host_arch() else {
        return Vec::new();
    };
    PRESETS
        .iter()
        .filter(|preset| preset.arch == arch)
        .map(|preset| ImagePreset {
            name: preset.name,
            version: preset.version,
            description: preset.description,
            url: preset.url,
            sha256: preset.sha256,
            arch: preset.arch,
        })
        .collect()
}

/// Returns true when `name` is a known preset family (any version/arch).
#[must_use]
pub fn is_known_preset_name(name: &str) -> bool {
    PRESETS.iter().any(|preset| preset.name == name)
        || UNSUPPORTED_PRESETS.iter().any(|(n, _)| *n == name)
}

fn split_name_version(location: &str) -> Result<(&str, &str)> {
    let location = location.trim().trim_start_matches('/');
    if location.is_empty() {
        return Err(ContainustError::Config {
            message: "preset:// reference is missing a name (example: preset://alpine)".into(),
        });
    }
    Ok(match location.split_once(':') {
        Some((name, version)) if !name.is_empty() && !version.is_empty() => (name, version),
        Some(_) => {
            return Err(ContainustError::Config {
                message: format!(
                    "invalid preset reference '{location}' \
                     (expected preset://name or preset://name:version)"
                ),
            });
        }
        None => (location, "latest"),
    })
}

fn host_arch() -> Result<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("x86_64"),
        "aarch64" => Ok("aarch64"),
        other => Err(ContainustError::Config {
            message: format!(
                "no curated presets for architecture '{other}' \
                 (supported: x86_64, aarch64)"
            ),
        }),
    }
}

fn unknown_preset(name: &str, version: &str, arch: &str) -> ContainustError {
    if let Some((_, hint)) = UNSUPPORTED_PRESETS.iter().find(|(n, _)| *n == name) {
        return ContainustError::Config {
            message: (*hint).into(),
        };
    }
    let available: Vec<String> = PRESETS
        .iter()
        .filter(|preset| preset.arch == arch)
        .map(|preset| format!("{}:{}", preset.name, preset.version))
        .collect();
    ContainustError::NotFound {
        kind: "image preset",
        id: format!(
            "{name}:{version} for {arch}. Available: {}",
            available.join(", ")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::ImageScheme;

    #[test]
    fn resolve_alpine_latest_selects_default_version() {
        let reference = ImageReference::parse("preset://alpine").expect("parse");
        assert_eq!(reference.scheme(), ImageScheme::Preset);
        let preset = resolve_preset(&reference).expect("resolve");
        assert_eq!(preset.name, "alpine");
        assert_eq!(preset.version, "3.22");
        assert_eq!(preset.sha256.len(), 64);
        assert!(preset.url.starts_with("https://dl-cdn.alpinelinux.org/"));
    }

    #[test]
    fn resolve_alpine_pin_selects_requested_version() {
        let reference = ImageReference::parse("preset://alpine:3.21").expect("parse");
        let preset = resolve_preset(&reference).expect("resolve");
        assert_eq!(preset.version, "3.21");
        assert!(preset.url.contains("v3.21"));
    }

    #[test]
    fn resolve_busybox_latest_succeeds() {
        let reference = ImageReference::parse("preset://busybox").expect("parse");
        let preset = resolve_preset(&reference).expect("resolve");
        assert_eq!(preset.name, "busybox");
    }

    #[test]
    fn resolve_node_returns_actionable_hint() {
        let reference = ImageReference::parse("preset://node").expect("parse");
        let error = resolve_preset(&reference).expect_err("node unsupported");
        assert!(error.to_string().contains("ctst pull"));
        assert!(error.to_string().contains("node"));
    }

    #[test]
    fn resolve_unknown_preset_lists_available() {
        let reference = ImageReference::parse("preset://does-not-exist").expect("parse");
        let error = resolve_preset(&reference).expect_err("unknown");
        assert!(error.to_string().contains("alpine"));
    }

    #[test]
    fn preset_fetch_reference_pins_digest() {
        let reference = ImageReference::parse("preset://alpine:3.22").expect("parse");
        let preset = resolve_preset(&reference).expect("resolve");
        let fetch = preset_fetch_reference(&preset).expect("fetch ref");
        assert!(fetch.is_remote());
        assert_eq!(fetch.digest().expect("digest").as_hex(), preset.sha256);
    }

    #[test]
    fn list_presets_only_returns_host_architecture() {
        let presets = list_presets();
        assert!(!presets.is_empty());
        let arch = host_arch().expect("arch");
        assert!(presets.iter().all(|preset| preset.arch == arch));
    }

    #[test]
    fn empty_preset_name_rejected() {
        let error = ImageReference::parse("preset://").expect_err("empty");
        assert!(error.to_string().contains("empty"));
    }
}
