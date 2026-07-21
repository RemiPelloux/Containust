//! Structured image references.
//!
//! An [`ImageReference`] carries the scheme, location, and optional
//! SHA-256 digest of an image source. Parsing is pure: it never touches
//! the filesystem or network, so references can be validated before any
//! I/O decision (including offline enforcement) is made.

use std::fmt;

use containust_common::error::{ContainustError, Result};
use containust_common::types::Sha256Hash;
use sha2::{Digest, Sha256};

/// Transport scheme of an image source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageScheme {
    /// Local directory rootfs (`file://`).
    File,
    /// Local tar / tar.gz archive (`tar://`).
    Tar,
    /// Remote HTTPS archive (`https://`), requires explicit opt-in.
    Https,
    /// Remote HTTP archive (`http://`), requires explicit opt-in.
    Http,
    /// Image imported into the local content-addressed catalog (`image://`).
    Catalog,
    /// Curated well-known rootfs (`preset://alpine`), resolved to a pinned download.
    Preset,
}

impl ImageScheme {
    /// Returns the URI prefix for this scheme.
    #[must_use]
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::File => "file://",
            Self::Tar => "tar://",
            Self::Https => "https://",
            Self::Http => "http://",
            Self::Catalog => "image://",
            Self::Preset => "preset://",
        }
    }

    /// Returns whether resolving this scheme requires network access.
    ///
    /// Presets download on first use, but are satisfied from the local
    /// layer store once imported — callers should treat them as
    /// "remote unless cached".
    #[must_use]
    pub const fn is_remote(self) -> bool {
        matches!(self, Self::Https | Self::Http | Self::Preset)
    }
}

/// A parsed, structured image reference.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageReference {
    scheme: ImageScheme,
    location: String,
    digest: Option<Sha256Hash>,
}

impl ImageReference {
    /// Parses an image URI such as `tar:///images/app.tar@sha256:<hex>`.
    ///
    /// The optional `@sha256:<hex>` suffix pins the expected content digest.
    /// Parsing performs no I/O.
    ///
    /// # Errors
    ///
    /// Returns an error if the scheme is unsupported, the location is
    /// empty, or the digest suffix is malformed.
    pub fn parse(uri: &str) -> Result<Self> {
        let (scheme, rest) = split_scheme(uri)?;
        let (location, digest) = split_digest(rest)?;
        if location.is_empty() {
            return Err(ContainustError::Config {
                message: format!("image reference has an empty location: {uri}"),
            });
        }
        Ok(Self {
            scheme,
            location: location.to_string(),
            digest,
        })
    }

    /// Returns the transport scheme.
    #[must_use]
    pub const fn scheme(&self) -> ImageScheme {
        self.scheme
    }

    /// Returns the path, URL host+path, or catalog name.
    #[must_use]
    pub fn location(&self) -> &str {
        &self.location
    }

    /// Returns the pinned content digest, if any.
    #[must_use]
    pub const fn digest(&self) -> Option<&Sha256Hash> {
        self.digest.as_ref()
    }

    /// Returns whether resolving this reference requires network access.
    #[must_use]
    pub const fn is_remote(&self) -> bool {
        self.scheme.is_remote()
    }

    /// Returns the deterministic local cache key for this reference.
    ///
    /// References pinned by digest share a cache entry regardless of
    /// where they were fetched from; unpinned references are keyed by
    /// the SHA-256 of their canonical URI.
    #[must_use]
    pub fn cache_key(&self) -> String {
        self.digest.as_ref().map_or_else(
            || {
                let digest = Sha256::digest(self.canonical_uri().as_bytes());
                format!("{digest:x}")
            },
            |digest| digest.as_hex().to_string(),
        )
    }

    /// Returns the canonical URI without any digest suffix.
    #[must_use]
    pub fn canonical_uri(&self) -> String {
        format!("{}{}", self.scheme.prefix(), self.location)
    }
}

impl fmt::Display for ImageReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.canonical_uri())?;
        if let Some(digest) = &self.digest {
            write!(f, "@sha256:{}", digest.as_hex())?;
        }
        Ok(())
    }
}

fn split_scheme(uri: &str) -> Result<(ImageScheme, &str)> {
    const SCHEMES: [ImageScheme; 6] = [
        ImageScheme::File,
        ImageScheme::Tar,
        ImageScheme::Https,
        ImageScheme::Http,
        ImageScheme::Catalog,
        ImageScheme::Preset,
    ];
    SCHEMES
        .into_iter()
        .find_map(|scheme| uri.strip_prefix(scheme.prefix()).map(|rest| (scheme, rest)))
        .ok_or_else(|| ContainustError::Config {
            message: format!(
                "unsupported image source URI scheme: {uri} \
                 (expected file://, tar://, image://, preset://, https://, or http://)"
            ),
        })
}

fn split_digest(rest: &str) -> Result<(&str, Option<Sha256Hash>)> {
    match rest.rsplit_once("@sha256:") {
        None => Ok((rest, None)),
        Some((location, hex)) => {
            let digest = Sha256Hash::from_hex(hex)?;
            Ok((location, Some(digest)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";

    #[test]
    fn parse_file_reference_extracts_path() {
        let reference = ImageReference::parse("file:///images/app").expect("parse");
        assert_eq!(reference.scheme(), ImageScheme::File);
        assert_eq!(reference.location(), "/images/app");
        assert!(reference.digest().is_none());
    }

    #[test]
    fn parse_tar_reference_with_digest_pins_hash() {
        let uri = format!("tar:///images/app.tar@sha256:{DIGEST}");
        let reference = ImageReference::parse(&uri).expect("parse");
        assert_eq!(reference.scheme(), ImageScheme::Tar);
        assert_eq!(reference.location(), "/images/app.tar");
        assert_eq!(reference.digest().expect("digest").as_hex(), DIGEST);
    }

    #[test]
    fn parse_catalog_reference_extracts_name() {
        let reference = ImageReference::parse("image://web").expect("parse");
        assert_eq!(reference.scheme(), ImageScheme::Catalog);
        assert_eq!(reference.location(), "web");
        assert!(!reference.is_remote());
    }

    #[test]
    fn parse_preset_reference_extracts_name_and_is_remote() {
        let reference = ImageReference::parse("preset://alpine:3.22").expect("parse");
        assert_eq!(reference.scheme(), ImageScheme::Preset);
        assert_eq!(reference.location(), "alpine:3.22");
        assert!(reference.is_remote());
    }

    #[test]
    fn parse_https_reference_is_remote() {
        let reference = ImageReference::parse("https://example.test/a.tar").expect("parse");
        assert!(reference.is_remote());
    }

    #[test]
    fn parse_unknown_scheme_returns_error() {
        assert!(ImageReference::parse("ftp://example.test/a.tar").is_err());
    }

    #[test]
    fn parse_empty_location_returns_error() {
        assert!(ImageReference::parse("file://").is_err());
    }

    #[test]
    fn parse_invalid_digest_returns_error() {
        assert!(ImageReference::parse("tar:///a.tar@sha256:zzzz").is_err());
    }

    #[test]
    fn cache_key_uses_pinned_digest_when_present() {
        let uri = format!("https://example.test/a.tar@sha256:{DIGEST}");
        let reference = ImageReference::parse(&uri).expect("parse");
        assert_eq!(reference.cache_key(), DIGEST);
    }

    #[test]
    fn cache_key_without_digest_is_deterministic() {
        let first = ImageReference::parse("file:///images/app").expect("parse");
        let second = ImageReference::parse("file:///images/app").expect("parse");
        assert_eq!(first.cache_key(), second.cache_key());
        assert_eq!(first.cache_key().len(), 64);
    }

    #[test]
    fn display_round_trips_digest_suffix() {
        let uri = format!("image://web@sha256:{DIGEST}");
        let reference = ImageReference::parse(&uri).expect("parse");
        assert_eq!(reference.to_string(), uri);
    }
}
