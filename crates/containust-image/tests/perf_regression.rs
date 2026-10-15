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

/// Generous wall-clock budget for one 32 MiB import (observed ~100 ms on
/// Linux/macOS; Windows CI runners with Defender scanning have exceeded
/// 5 s, so the budget only guards against gross regressions).
const IMPORT_BUDGET: Duration = Duration::from_secs(20);

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

/// Number of tiny entries used by the many-small-files extraction gate.
const SMALL_FILE_COUNT: usize = 10_000;

/// Generous budget for extracting `SMALL_FILE_COUNT` gzip entries (observed
/// well under 1 s locally); guards against per-entry reopen/unbuffered I/O.
const SMALL_FILES_BUDGET: Duration = Duration::from_secs(20);

#[test]
fn extract_many_small_files_completes_within_budget() {
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = dir.path().join("small.tar.gz");
    {
        let file = std::fs::File::create(&archive).expect("create archive");
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        let mut builder = tar::Builder::new(encoder);
        for index in 0..SMALL_FILE_COUNT {
            let data = format!("payload-{index}\n");
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(
                    &mut header,
                    format!("d{}/f{index}", index % 64),
                    data.as_bytes(),
                )
                .expect("append");
        }
        let _ = builder
            .into_inner()
            .expect("tar finish")
            .finish()
            .expect("gzip finish");
    }
    let target = dir.path().join("out");

    let start = Instant::now();
    containust_image::extract::safe_extract_archive(&archive, &target).expect("extract");
    let elapsed = start.elapsed();

    assert!(
        elapsed < SMALL_FILES_BUDGET,
        "{SMALL_FILE_COUNT} small-file extraction took {elapsed:?}, budget is {SMALL_FILES_BUDGET:?}"
    );
    let last = SMALL_FILE_COUNT - 1;
    assert_eq!(
        std::fs::read_to_string(target.join(format!("d{}/f{last}", last % 64))).expect("read"),
        format!("payload-{last}\n")
    );
}
