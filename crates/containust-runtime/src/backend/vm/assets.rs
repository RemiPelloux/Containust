//! Pinned VM boot assets (kernel + base initramfs).
//!
//! Each architecture maps to versioned Alpine netboot URLs with
//! committed SHA-256 digests. Downloads are resumable, cache updates
//! are exclusive-locked, and offline mode fails closed with a clear
//! remediation hint.

use std::path::Path;

use containust_common::error::{ContainustError, Result};
use containust_common::types::Sha256Hash;

use super::assets_fetch::{CacheLock, download_resumable};

/// Network policy for populating the VM asset cache.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AssetCachePolicy {
    /// When true, refuse downloads and require a verified local cache.
    pub offline: bool,
}

/// Pinned VM boot assets for one host architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmAssetEntry {
    /// Host architecture (`x86_64` or `aarch64`).
    pub arch: &'static str,
    /// Exact Alpine patch release used for the netboot tree.
    pub alpine_release: &'static str,
    /// Versioned URL of the virt kernel.
    pub kernel_url: &'static str,
    /// SHA-256 of the kernel blob (64 hex chars).
    pub kernel_sha256: &'static str,
    /// Versioned URL of the virt initramfs.
    pub initramfs_url: &'static str,
    /// SHA-256 of the initramfs blob (64 hex chars).
    pub initramfs_sha256: &'static str,
}

/// Curated, digest-pinned Alpine netboot assets.
pub const VM_ASSETS: &[VmAssetEntry] = &[
    VmAssetEntry {
        arch: "x86_64",
        alpine_release: "3.21.7",
        kernel_url: "https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/x86_64/netboot-3.21.7/vmlinuz-virt",
        kernel_sha256: "26bf81ada3e8fc30fd4d81805fe6c8c60be5c7fb18a43563c707e49117e624ca",
        initramfs_url: "https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/x86_64/netboot-3.21.7/initramfs-virt",
        initramfs_sha256: "e2562e019a506f9bdac24d06953823106a2ab29da50eea01185d005a3ca4acdf",
    },
    VmAssetEntry {
        arch: "aarch64",
        alpine_release: "3.21.7",
        kernel_url: "https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/aarch64/netboot-3.21.7/vmlinuz-virt",
        kernel_sha256: "749eb77d8c0a887868166c220e36411400b9bed5df6443b201c96950faf0f8ac",
        initramfs_url: "https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/aarch64/netboot-3.21.7/initramfs-virt",
        initramfs_sha256: "6f48e46367737f1f223f2be3968945e4aeb0e7089f87386aee9da967c46d6269",
    },
];

/// Returns the host architecture string used by the asset catalog.
#[must_use]
pub const fn host_arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x86_64"
    }
}

/// Looks up the pinned asset entry for `arch`.
///
/// # Errors
///
/// Returns an error when the architecture is not in the catalog.
pub fn asset_for_arch(arch: &str) -> Result<&'static VmAssetEntry> {
    VM_ASSETS
        .iter()
        .find(|entry| entry.arch == arch)
        .ok_or_else(|| ContainustError::Config {
            message: format!(
                "no pinned VM boot assets for architecture '{arch}' \
                 (supported: x86_64, aarch64)"
            ),
        })
}

/// Ensures kernel and initramfs files exist at `dest_*` and match the
/// pinned digests. Downloads when missing or corrupt unless offline.
///
/// Concurrent callers serialize on an exclusive lock in the cache
/// directory so partial files are never corrupted.
///
/// # Errors
///
/// Returns an error if the architecture is unknown, offline mode blocks
/// a required download, the download fails, or the digest does not match.
pub fn ensure_cached(
    entry: &VmAssetEntry,
    dest_kernel: &Path,
    dest_initramfs: &Path,
    policy: AssetCachePolicy,
) -> Result<()> {
    let cache_dir = dest_kernel
        .parent()
        .ok_or_else(|| ContainustError::Config {
            message: format!(
                "VM kernel path has no parent directory: {}",
                dest_kernel.display()
            ),
        })?;
    let _lock = CacheLock::acquire(cache_dir)?;
    ensure_one(EnsureOne {
        kind: "kernel",
        url: entry.kernel_url,
        expected_hex: entry.kernel_sha256,
        dest: dest_kernel,
        policy,
    })?;
    ensure_one(EnsureOne {
        kind: "initramfs",
        url: entry.initramfs_url,
        expected_hex: entry.initramfs_sha256,
        dest: dest_initramfs,
        policy,
    })?;
    Ok(())
}

#[derive(Clone, Copy)]
struct EnsureOne<'a> {
    kind: &'a str,
    url: &'a str,
    expected_hex: &'a str,
    dest: &'a Path,
    policy: AssetCachePolicy,
}

fn ensure_one(req: EnsureOne<'_>) -> Result<()> {
    let expected = Sha256Hash::from_hex(req.expected_hex)?;
    if req.dest.exists() && !is_empty(req.dest) {
        match containust_image::hash::validate_hash(req.dest, &expected) {
            Ok(()) => return Ok(()),
            Err(error) => {
                tracing::warn!(
                    path = %req.dest.display(),
                    %error,
                    "cached VM {} failed digest check; re-downloading",
                    req.kind
                );
                let _ = std::fs::remove_file(req.dest);
            }
        }
    }
    if req.policy.offline {
        return Err(ContainustError::Network {
            url: req.url.to_string(),
            message: format!(
                "offline mode blocks VM {} download. Run once online \
                 (`ctst vm start` without --offline) to populate \
                 ~/.containust/cache/vm/, then retry air-gapped",
                req.kind
            ),
        });
    }
    eprintln!(
        "  Downloading Alpine Linux {} (first run / digest refresh)...",
        req.kind
    );
    download_resumable(req.url, req.dest, &expected)
}

fn is_empty(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|meta| meta.len() == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_for_arch_x86_64_returns_pinned_urls() {
        let entry = asset_for_arch("x86_64").expect("x86_64");
        assert!(entry.kernel_url.contains("netboot-3.21.7"));
        assert!(entry.kernel_url.contains("x86_64"));
        assert_eq!(entry.alpine_release, "3.21.7");
    }

    #[test]
    fn asset_for_arch_unknown_fails_closed() {
        let error = asset_for_arch("riscv64").expect_err("unsupported");
        assert!(error.to_string().contains("riscv64"));
    }

    #[test]
    fn asset_entries_have_64_char_hex_digests() {
        for entry in VM_ASSETS {
            assert_eq!(entry.kernel_sha256.len(), 64);
            assert_eq!(entry.initramfs_sha256.len(), 64);
            assert!(entry.kernel_sha256.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(
                entry
                    .initramfs_sha256
                    .chars()
                    .all(|c| c.is_ascii_hexdigit())
            );
            assert!(Sha256Hash::from_hex(entry.kernel_sha256).is_ok());
            assert!(Sha256Hash::from_hex(entry.initramfs_sha256).is_ok());
        }
    }

    const KERNEL_BYTES: &[u8] = b"kernel-bytes";
    const INIT_BYTES: &[u8] = b"init-bytes";
    const KERNEL_DIGEST: &str = "4e72696f3eefb3b2375c36063864c2635cf3b8c85a83296a9cc30b0534c16f4d";
    const INIT_DIGEST: &str = "d04f51788dae997f43d3ae7614982f99ce1b0a184a791ee42f308df491e669ff";
    const ZERO_DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    #[test]
    fn ensure_cached_accepts_matching_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let kernel = dir.path().join("vmlinuz");
        let initramfs = dir.path().join("initramfs");
        std::fs::write(&kernel, KERNEL_BYTES).expect("write kernel");
        std::fs::write(&initramfs, INIT_BYTES).expect("write init");
        let entry = VmAssetEntry {
            arch: "test",
            alpine_release: "0.0.0",
            kernel_url: "http://127.0.0.1:1/unused",
            kernel_sha256: KERNEL_DIGEST,
            initramfs_url: "http://127.0.0.1:1/unused",
            initramfs_sha256: INIT_DIGEST,
        };
        ensure_cached(
            &entry,
            &kernel,
            &initramfs,
            AssetCachePolicy { offline: true },
        )
        .expect("matching cache is accepted offline");
    }

    #[test]
    fn ensure_cached_offline_missing_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let entry = VmAssetEntry {
            arch: "test",
            alpine_release: "0.0.0",
            kernel_url: "http://127.0.0.1:1/missing-kernel",
            kernel_sha256: ZERO_DIGEST,
            initramfs_url: "http://127.0.0.1:1/missing-initramfs",
            initramfs_sha256: ZERO_DIGEST,
        };
        let error = ensure_cached(
            &entry,
            &dir.path().join("vmlinuz"),
            &dir.path().join("initramfs"),
            AssetCachePolicy { offline: true },
        )
        .expect_err("offline missing must fail");
        assert!(error.to_string().contains("offline"));
        assert!(error.to_string().contains("cache/vm"));
    }

    #[test]
    fn ensure_cached_offline_corrupt_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let kernel = dir.path().join("vmlinuz");
        let initramfs = dir.path().join("initramfs");
        std::fs::write(&kernel, b"not-a-kernel").expect("write");
        std::fs::write(&initramfs, b"not-an-initramfs").expect("write");
        let entry = VmAssetEntry {
            arch: "test",
            alpine_release: "0.0.0",
            kernel_url: "https://example.test/kernel",
            kernel_sha256: ZERO_DIGEST,
            initramfs_url: "https://example.test/initramfs",
            initramfs_sha256: ZERO_DIGEST,
        };
        let error = ensure_cached(
            &entry,
            &kernel,
            &initramfs,
            AssetCachePolicy { offline: true },
        )
        .expect_err("offline corrupt must fail");
        assert!(error.to_string().contains("offline"));
    }
}
