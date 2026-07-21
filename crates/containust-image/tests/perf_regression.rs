//! Performance regression gates for the image import pipeline.
//!
//! Budgets are deliberately generous (they must never flake on slow CI
//! machines) but tight enough to catch a regression back to multi-pass
//! hashing or accidental extra copies of layer blobs.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use containust_image::import::{ImportRequest, import_image};
use containust_image::reference::ImageReference;

/// Size of the synthetic payload used for timing gates.
const PAYLOAD_BYTES: usize = 32 * 1024 * 1024;

/// Generous wall-clock budget for one 32 MiB import (observed ~100 ms).
const IMPORT_BUDGET: Duration = Duration::from_secs(5);

/// Writes a rootfs with one incompressible `PAYLOAD_BYTES` blob.
fn build_payload_rootfs(root: &Path) {
    std::fs::create_dir_all(root.join("data")).expect("mkdir");
    let mut file = std::fs::File::create(root.join("data/blob.bin")).expect("create blob");
    // Pseudo-random bytes so the payload is not trivially sparse.
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut chunk = vec![0_u8; 64 * 1024];
    let mut remaining = PAYLOAD_BYTES;
    while remaining > 0 {
        for byte in &mut chunk {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            *byte = state.to_le_bytes()[4];
        }
        let take = remaining.min(chunk.len());
        file.write_all(&chunk[..take]).expect("write blob");
        remaining -= take;
    }
}

/// Lists leftover staging files in the layer store.
fn staging_leftovers(data_dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(data_dir.join("layers")) else {
        return Vec::new();
    };
    entries
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(".staging-"))
        .collect()
}

#[test]
fn import_directory_32mib_completes_within_budget() {
    let dir = tempfile::tempdir().expect("tempdir");
    let rootfs = dir.path().join("rootfs");
    build_payload_rootfs(&rootfs);
    let data_dir = dir.path().join("data");
    let reference =
        ImageReference::parse(&format!("file://{}", rootfs.display())).expect("parse reference");

    let start = Instant::now();
    let entry = import_image(&data_dir, &reference, &ImportRequest::new("perf", false))
        .expect("import succeeds");
    let elapsed = start.elapsed();

    assert!(
        elapsed < IMPORT_BUDGET,
        "32 MiB directory import took {elapsed:?}, budget is {IMPORT_BUDGET:?}"
    );
    assert!(entry.size_bytes >= PAYLOAD_BYTES as u64);
}

#[test]
fn import_tar_32mib_completes_within_budget() {
    let dir = tempfile::tempdir().expect("tempdir");
    let rootfs = dir.path().join("rootfs");
    build_payload_rootfs(&rootfs);
    let archive = dir.path().join("image.tar");
    containust_image::pack::pack_directory(&rootfs, &archive).expect("pack");
    let data_dir = dir.path().join("data");
    let reference =
        ImageReference::parse(&format!("tar://{}", archive.display())).expect("parse reference");

    let start = Instant::now();
    let entry = import_image(&data_dir, &reference, &ImportRequest::new("perf", true))
        .expect("import succeeds");
    let elapsed = start.elapsed();

    assert!(
        elapsed < IMPORT_BUDGET,
        "32 MiB tar import took {elapsed:?}, budget is {IMPORT_BUDGET:?}"
    );
    assert_eq!(entry.digest.as_deref().map(str::len), Some(64));
}

#[test]
fn repeated_imports_leave_no_staging_files_behind() {
    let dir = tempfile::tempdir().expect("tempdir");
    let rootfs = dir.path().join("rootfs");
    build_payload_rootfs(&rootfs);
    let data_dir = dir.path().join("data");
    let reference =
        ImageReference::parse(&format!("file://{}", rootfs.display())).expect("parse reference");
    let request = ImportRequest::new("perf", false);

    let first = import_image(&data_dir, &reference, &request).expect("first import");
    let second = import_image(&data_dir, &reference, &request).expect("second import");

    assert_eq!(first.digest, second.digest);
    let leftovers = staging_leftovers(&data_dir);
    assert!(
        leftovers.is_empty(),
        "staging files leaked into the layer store: {leftovers:?}"
    );
}
