//! Content-addressed image import and materialization.
//!
//! Importing converts any supported source (directory, tar archive, or
//! opt-in remote download) into a verified layer blob addressed by its
//! SHA-256 digest, then records the image in the local catalog with its
//! supply-chain metadata. Materialization reconstructs a rootfs from
//! the catalog without touching the original source or the network.

use std::io::Read;
use std::path::{Path, PathBuf};

use containust_common::error::{ContainustError, Result};
use containust_common::types::{ImageId, Sha256Hash};

use crate::fetch::{FetchPolicy, fetch_remote};
use crate::pack::pack_directory_hashed;
use crate::preset::{preset_fetch_reference, resolve_preset};
use crate::reference::{ImageReference, ImageScheme};
use crate::registry::{ImageCatalog, ImageEntry};
use crate::storage::StorageBackend;

const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

/// Parameters for importing an image into the local store.
#[derive(Debug, Clone)]
pub struct ImportRequest {
    /// Catalog name to register the image under.
    pub name: String,
    /// When true, remote sources are rejected before any connection.
    pub offline: bool,
    /// Network policy applied to remote sources.
    pub fetch_policy: FetchPolicy,
}

impl ImportRequest {
    /// Creates an import request with the default fetch policy.
    #[must_use]
    pub fn new(name: impl Into<String>, offline: bool) -> Self {
        Self {
            name: name.into(),
            offline,
            fetch_policy: FetchPolicy {
                offline,
                ..FetchPolicy::default()
            },
        }
    }
}

/// Imports an image source into the content-addressed local store.
///
/// The resulting layer digest is deterministic: importing the same
/// directory or archive twice always produces the same content address
/// and reuses the stored layer.
///
/// # Errors
///
/// Returns an error if the source is missing, a pinned digest does not
/// match, offline mode blocks a remote source, or storage/catalog
/// operations fail.
pub fn import_image(
    data_dir: &Path,
    reference: &ImageReference,
    request: &ImportRequest,
) -> Result<ImageEntry> {
    let store = StorageBackend::open(data_dir.to_path_buf())?;
    let staged = stage_source(&store, reference, request)?;

    let digest = staged.digest().clone();
    if let Some(pinned) = reference.digest()
        && pinned.as_hex() != digest.as_hex()
    {
        staged.discard();
        return Err(ContainustError::HashMismatch {
            resource: reference.to_string(),
            expected: pinned.as_hex().to_string(),
            actual: digest.as_hex().to_string(),
        });
    }

    let size_bytes = match staged {
        StagedLayer::Staged { ref path, .. } => {
            let size = file_size(path)?;
            store.commit_layer(path, digest.as_hex())?;
            size
        }
        StagedLayer::Cached { .. } => file_size(&store.layer_blob_path(digest.as_hex()))?,
    };

    let entry = ImageEntry {
        id: ImageId::new(digest.as_hex()),
        name: request.name.clone(),
        source: reference.to_string(),
        layers: vec![digest.as_hex().to_string()],
        size_bytes,
        created_at: chrono::Utc::now().to_rfc3339(),
        digest: Some(digest.as_hex().to_string()),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    ImageCatalog::open(data_dir)?.register(entry.clone())?;
    tracing::info!(name = %entry.name, digest = %digest, "image imported");
    Ok(entry)
}

/// Reconstructs an image rootfs from the local catalog into `target`.
///
/// Works entirely from the content-addressed store, so it is safe in
/// offline / air-gapped environments. A digest pinned on the reference
/// must match the catalog entry.
///
/// # Errors
///
/// Returns an error if the image or one of its layers is missing, the
/// pinned digest disagrees with the catalog, or extraction fails.
pub fn materialize_image(data_dir: &Path, reference: &ImageReference, target: &Path) -> Result<()> {
    if reference.scheme() != ImageScheme::Catalog {
        return Err(ContainustError::Config {
            message: format!(
                "only image:// references can be materialized from the catalog, got: {reference}"
            ),
        });
    }
    let entry = ImageCatalog::open(data_dir)?.find(reference.location())?;
    verify_pinned_digest(reference, &entry)?;

    let store = StorageBackend::open(data_dir.to_path_buf())?;
    std::fs::create_dir_all(target).map_err(|source| ContainustError::Io {
        path: target.to_path_buf(),
        source,
    })?;
    for layer in &entry.layers {
        extract_layer_blob(&store, layer, target)?;
    }
    tracing::info!(name = %entry.name, target = %target.display(), "image materialized");
    Ok(())
}

fn verify_pinned_digest(reference: &ImageReference, entry: &ImageEntry) -> Result<()> {
    let Some(pinned) = reference.digest() else {
        return Ok(());
    };
    if entry.digest.as_deref() == Some(pinned.as_hex()) {
        return Ok(());
    }
    Err(ContainustError::HashMismatch {
        resource: reference.to_string(),
        expected: pinned.as_hex().to_string(),
        actual: entry.digest.clone().unwrap_or_else(|| "<none>".into()),
    })
}

/// A layer produced by staging: either a fresh candidate blob whose
/// digest was computed while it was written (single pass), or a
/// verified blob already present in the content-addressed store.
#[derive(Debug)]
enum StagedLayer {
    /// A candidate blob awaiting commit into the store.
    Staged { path: PathBuf, digest: Sha256Hash },
    /// The layer already exists in the store; no copy is needed.
    Cached { digest: Sha256Hash },
}

impl StagedLayer {
    const fn digest(&self) -> &Sha256Hash {
        match self {
            Self::Staged { digest, .. } | Self::Cached { digest } => digest,
        }
    }

    /// Removes the staged file, if any, after a failed import.
    fn discard(&self) {
        if let Self::Staged { path, .. } = self {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn stage_source(
    store: &StorageBackend,
    reference: &ImageReference,
    request: &ImportRequest,
) -> Result<StagedLayer> {
    if request.offline && reference.is_remote() && reference.scheme() != ImageScheme::Preset {
        return Err(ContainustError::Network {
            url: reference.canonical_uri(),
            message: "offline mode blocks remote image import".into(),
        });
    }
    let staged = store.staging_path();
    let digest = match reference.scheme() {
        ImageScheme::File => {
            let source = require_existing(reference.location(), "image directory")?;
            pack_directory_hashed(&source, &staged)?
        }
        ImageScheme::Tar => {
            // `fs::copy` clones on reflink filesystems (APFS, btrfs) and
            // uses in-kernel copy elsewhere; hashing the staged copy is
            // then the only userspace pass over the bytes.
            let source = require_existing(reference.location(), "tar archive")?;
            let _ = std::fs::copy(&source, &staged).map_err(|e| ContainustError::Io {
                path: source.clone(),
                source: e,
            })?;
            crate::hash::hash_file(&staged)?
        }
        ImageScheme::Https | ImageScheme::Http => {
            fetch_remote(reference, &request.fetch_policy, &staged)?
        }
        ImageScheme::Preset => return stage_preset(store, reference, request, &staged),
        ImageScheme::Catalog => {
            return Err(ContainustError::Config {
                message: format!(
                    "image:// references are already imported and cannot be re-imported: \
                     {reference}"
                ),
            });
        }
    };
    Ok(StagedLayer::Staged {
        path: staged,
        digest,
    })
}

/// Stages a curated preset from the local layer cache, or downloads it.
///
/// A cached blob is integrity-checked against the curated digest and
/// then reused in place — no staging copy, no re-import of bytes.
fn stage_preset(
    store: &StorageBackend,
    reference: &ImageReference,
    request: &ImportRequest,
    staged: &Path,
) -> Result<StagedLayer> {
    let preset = resolve_preset(reference)?;
    let curated = Sha256Hash::from_hex(preset.sha256)?;
    if store.has_layer(preset.sha256) {
        let blob = store.layer_blob_path(preset.sha256);
        crate::hash::validate_hash(&blob, &curated)?;
        tracing::info!(
            name = preset.name,
            version = preset.version,
            "preset satisfied from local layer cache"
        );
        return Ok(StagedLayer::Cached { digest: curated });
    }
    if request.offline {
        return Err(ContainustError::Network {
            url: reference.canonical_uri(),
            message: format!(
                "offline mode: preset '{}:{}' is not cached. Import it once online \
                 with `ctst build`, then reuse the project store air-gapped",
                preset.name, preset.version
            ),
        });
    }
    let fetch_ref = preset_fetch_reference(&preset)?;
    let mut policy = request.fetch_policy.clone();
    policy.offline = false;
    let digest = fetch_remote(&fetch_ref, &policy, staged)?;
    tracing::info!(
        name = preset.name,
        version = preset.version,
        digest = preset.sha256,
        "preset downloaded and verified"
    );
    Ok(StagedLayer::Staged {
        path: staged.to_path_buf(),
        digest,
    })
}

fn require_existing(location: &str, kind: &'static str) -> Result<std::path::PathBuf> {
    let path = std::path::PathBuf::from(location);
    if !path.exists() {
        return Err(ContainustError::NotFound {
            kind,
            id: location.to_string(),
        });
    }
    Ok(path)
}

fn file_size(path: &Path) -> Result<u64> {
    let metadata = std::fs::metadata(path).map_err(|source| ContainustError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(metadata.len())
}

fn extract_layer_blob(store: &StorageBackend, hash: &str, target: &Path) -> Result<()> {
    let blob = store.layer_blob_path(hash);
    let mut file = std::fs::File::open(&blob).map_err(|_| ContainustError::NotFound {
        kind: "image layer",
        id: hash.to_string(),
    })?;
    let mut magic = [0_u8; 2];
    let gzip = file
        .read(&mut magic)
        .map_err(|source| ContainustError::Io {
            path: blob.clone(),
            source,
        })?
        == 2
        && magic == GZIP_MAGIC;

    let file = std::fs::File::open(&blob).map_err(|source| ContainustError::Io {
        path: blob.clone(),
        source,
    })?;
    let io_error = |source| ContainustError::Io {
        path: target.to_path_buf(),
        source,
    };
    if gzip {
        tar::Archive::new(flate2::read::GzDecoder::new(file))
            .unpack(target)
            .map_err(io_error)?;
    } else {
        tar::Archive::new(file).unpack(target).map_err(io_error)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_rootfs(root: &Path) {
        std::fs::create_dir_all(root.join("bin")).expect("mkdir");
        std::fs::write(root.join("bin/app"), b"#!/bin/sh\necho hi\n").expect("write");
    }

    fn file_reference(path: &Path) -> ImageReference {
        ImageReference::parse(&format!("file://{}", path.display())).expect("parse")
    }

    #[test]
    fn import_directory_twice_is_deterministic_and_deduplicated() {
        let dir = tempfile::tempdir().expect("tempdir");
        let rootfs = dir.path().join("rootfs");
        build_rootfs(&rootfs);
        let data_dir = dir.path().join("data");
        let request = ImportRequest::new("app", false);

        let first =
            import_image(&data_dir, &file_reference(&rootfs), &request).expect("first import");
        let second =
            import_image(&data_dir, &file_reference(&rootfs), &request).expect("second import");

        assert_eq!(first.digest, second.digest);
        assert_eq!(first.layers, second.layers);
        let catalog = ImageCatalog::open(&data_dir).expect("open catalog");
        assert_eq!(catalog.list().expect("list").len(), 1);
    }

    #[test]
    fn import_tar_records_supply_chain_metadata() {
        let dir = tempfile::tempdir().expect("tempdir");
        let rootfs = dir.path().join("rootfs");
        build_rootfs(&rootfs);
        let archive = dir.path().join("app.tar");
        crate::pack::pack_directory(&rootfs, &archive).expect("pack");
        let data_dir = dir.path().join("data");
        let reference =
            ImageReference::parse(&format!("tar://{}", archive.display())).expect("parse");

        let entry =
            import_image(&data_dir, &reference, &ImportRequest::new("app", true)).expect("import");

        assert_eq!(entry.digest.as_deref().map(str::len), Some(64));
        assert_eq!(entry.tool_version, env!("CARGO_PKG_VERSION"));
        assert!(entry.source.starts_with("tar://"));
        assert!(!entry.created_at.is_empty());
    }

    #[test]
    fn import_with_wrong_pinned_digest_fails_and_stores_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let rootfs = dir.path().join("rootfs");
        build_rootfs(&rootfs);
        let data_dir = dir.path().join("data");
        let wrong = "0".repeat(64);
        let reference =
            ImageReference::parse(&format!("file://{}@sha256:{wrong}", rootfs.display()))
                .expect("parse");

        let error = import_image(&data_dir, &reference, &ImportRequest::new("app", false))
            .expect_err("pinned mismatch must fail");

        assert!(matches!(error, ContainustError::HashMismatch { .. }));
        let catalog = ImageCatalog::open(&data_dir).expect("open catalog");
        assert!(catalog.list().expect("list").is_empty());
    }

    #[test]
    fn import_offline_rejects_remote_reference() {
        let dir = tempfile::tempdir().expect("tempdir");
        let digest = "0".repeat(64);
        let reference =
            ImageReference::parse(&format!("https://example.test/app.tar@sha256:{digest}"))
                .expect("parse");

        let error = import_image(dir.path(), &reference, &ImportRequest::new("app", true))
            .expect_err("offline remote must fail");

        assert!(error.to_string().contains("offline"));
    }

    #[test]
    fn import_missing_directory_returns_not_found() {
        let dir = tempfile::tempdir().expect("tempdir");
        let reference = file_reference(&dir.path().join("missing"));
        let error = import_image(dir.path(), &reference, &ImportRequest::new("app", false))
            .expect_err("missing source must fail");
        assert!(matches!(error, ContainustError::NotFound { .. }));
    }

    #[test]
    fn materialize_reconstructs_rootfs_from_catalog_only() {
        let dir = tempfile::tempdir().expect("tempdir");
        let rootfs = dir.path().join("rootfs");
        build_rootfs(&rootfs);
        let data_dir = dir.path().join("data");
        let entry = import_image(
            &data_dir,
            &file_reference(&rootfs),
            &ImportRequest::new("app", false),
        )
        .expect("import");

        // Delete the original source: materialization must not need it.
        std::fs::remove_dir_all(&rootfs).expect("remove source");

        let digest = entry.digest.expect("digest");
        let reference =
            ImageReference::parse(&format!("image://app@sha256:{digest}")).expect("parse");
        let target = dir.path().join("materialized");
        materialize_image(&data_dir, &reference, &target).expect("materialize");

        let content = std::fs::read(target.join("bin/app")).expect("read app");
        assert_eq!(content, b"#!/bin/sh\necho hi\n");
    }

    #[test]
    fn materialize_with_wrong_pinned_digest_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let rootfs = dir.path().join("rootfs");
        build_rootfs(&rootfs);
        let data_dir = dir.path().join("data");
        let _ = import_image(
            &data_dir,
            &file_reference(&rootfs),
            &ImportRequest::new("app", false),
        )
        .expect("import");

        let wrong = "0".repeat(64);
        let reference =
            ImageReference::parse(&format!("image://app@sha256:{wrong}")).expect("parse");
        let error = materialize_image(&data_dir, &reference, &dir.path().join("out"))
            .expect_err("pinned mismatch must fail");
        assert!(matches!(error, ContainustError::HashMismatch { .. }));
    }

    #[test]
    fn materialize_unknown_image_returns_not_found() {
        let dir = tempfile::tempdir().expect("tempdir");
        let reference = ImageReference::parse("image://ghost").expect("parse");
        let error = materialize_image(dir.path(), &reference, &dir.path().join("out"))
            .expect_err("unknown image must fail");
        assert!(matches!(error, ContainustError::NotFound { .. }));
    }

    #[test]
    fn reimporting_catalog_reference_is_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let reference = ImageReference::parse("image://app").expect("parse");
        let error = import_image(dir.path(), &reference, &ImportRequest::new("app", false))
            .expect_err("catalog re-import must fail");
        assert!(error.to_string().contains("cannot be re-imported"));
    }

    #[test]
    fn import_offline_preset_without_cache_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let reference = ImageReference::parse("preset://alpine").expect("parse");
        let error = import_image(dir.path(), &reference, &ImportRequest::new("app", true))
            .expect_err("uncached offline preset must fail");
        assert!(error.to_string().contains("offline"));
        assert!(error.to_string().contains("not cached"));
    }

    #[test]
    fn import_offline_preset_reuses_verified_layer_cache() {
        let dir = tempfile::tempdir().expect("tempdir");
        let data_dir = dir.path().join("data");
        let reference = ImageReference::parse("preset://alpine").expect("parse");
        let preset = crate::preset::resolve_preset(&reference).expect("resolve");

        // Plant a decoy blob under the curated digest name; integrity check
        // must reject it before offline import proceeds.
        let store = StorageBackend::open(data_dir.clone()).expect("open store");
        let staged = store.staging_path();
        std::fs::write(&staged, b"not-the-alpine-rootfs").expect("write decoy");
        store
            .commit_layer(&staged, preset.sha256)
            .expect("commit decoy under curated name");

        let error = import_image(&data_dir, &reference, &ImportRequest::new("app", true))
            .expect_err("corrupt cache must fail hash validation");
        assert!(matches!(error, ContainustError::HashMismatch { .. }));
    }
}
