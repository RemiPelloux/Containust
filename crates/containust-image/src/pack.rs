//! Deterministic directory packing.
//!
//! Converts a rootfs directory into a canonical tar archive whose bytes
//! depend only on the directory contents: entries are sorted, timestamps
//! are zeroed, and ownership is normalized to root. Importing the same
//! directory twice therefore always yields the same content address.

use std::path::{Path, PathBuf};

use containust_common::error::{ContainustError, Result};
use containust_common::types::Sha256Hash;

use crate::hash::HashingWriter;

/// Packs `source` into a deterministic tar archive at `destination`.
///
/// # Errors
///
/// Returns an error if the source cannot be read or the archive
/// cannot be written.
pub fn pack_directory(source: &Path, destination: &Path) -> Result<()> {
    let _ = pack_directory_hashed(source, destination)?;
    Ok(())
}

/// Packs `source` into a deterministic tar archive at `destination`,
/// returning the archive's SHA-256 digest computed in the same write
/// pass (no re-read of the produced file).
///
/// # Errors
///
/// Returns an error if the source cannot be read or the archive
/// cannot be written.
pub fn pack_directory_hashed(source: &Path, destination: &Path) -> Result<Sha256Hash> {
    let file = std::fs::File::create(destination).map_err(|source_err| ContainustError::Io {
        path: destination.to_path_buf(),
        source: source_err,
    })?;
    let mut builder = tar::Builder::new(HashingWriter::new(file));
    builder.follow_symlinks(false);

    for relative in collect_sorted_entries(source)? {
        append_entry(&mut builder, source, &relative)?;
    }

    let writer = builder
        .into_inner()
        .map_err(|source_err| ContainustError::Io {
            path: destination.to_path_buf(),
            source: source_err,
        })?;
    let (file, digest) = writer.finish()?;
    file.sync_all().map_err(|source_err| ContainustError::Io {
        path: destination.to_path_buf(),
        source: source_err,
    })?;
    Ok(digest)
}

/// Collects all entries under `root` as sorted relative paths.
fn collect_sorted_entries(root: &Path) -> Result<Vec<PathBuf>> {
    let mut entries = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let reader = std::fs::read_dir(&dir).map_err(|source| ContainustError::Io {
            path: dir.clone(),
            source,
        })?;
        for entry in reader {
            let entry = entry.map_err(|source| ContainustError::Io {
                path: dir.clone(),
                source,
            })?;
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|_| ContainustError::Config {
                    message: format!("entry escapes pack root: {}", path.display()),
                })?
                .to_path_buf();
            if path.is_dir() && !path.is_symlink() {
                pending.push(path);
            }
            entries.push(relative);
        }
    }
    entries.sort();
    Ok(entries)
}

/// Appends one normalized entry to the archive.
fn append_entry<W: std::io::Write>(
    builder: &mut tar::Builder<W>,
    root: &Path,
    relative: &Path,
) -> Result<()> {
    let absolute = root.join(relative);
    let io_error = |source| ContainustError::Io {
        path: absolute.clone(),
        source,
    };
    let metadata = std::fs::symlink_metadata(&absolute).map_err(io_error)?;

    let mut header = tar::Header::new_gnu();
    header.set_metadata_in_mode(&metadata, tar::HeaderMode::Deterministic);

    if metadata.is_symlink() {
        let target = std::fs::read_link(&absolute).map_err(io_error)?;
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        builder
            .append_link(&mut header, relative, &target)
            .map_err(io_error)?;
    } else if metadata.is_dir() {
        builder
            .append_data(&mut header, relative, std::io::empty())
            .map_err(io_error)?;
    } else {
        let file = std::fs::File::open(&absolute).map_err(io_error)?;
        builder
            .append_data(&mut header, relative, file)
            .map_err(io_error)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_fixture(root: &Path) {
        std::fs::create_dir_all(root.join("bin")).expect("mkdir bin");
        std::fs::create_dir_all(root.join("etc")).expect("mkdir etc");
        std::fs::write(root.join("bin/app"), b"#!/bin/sh\necho hi\n").expect("write app");
        std::fs::write(root.join("etc/config"), b"key=value\n").expect("write config");
    }

    #[test]
    fn pack_same_directory_twice_produces_identical_bytes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let rootfs = dir.path().join("rootfs");
        build_fixture(&rootfs);

        let first = dir.path().join("first.tar");
        let second = dir.path().join("second.tar");
        pack_directory(&rootfs, &first).expect("pack first");
        pack_directory(&rootfs, &second).expect("pack second");

        let first_bytes = std::fs::read(&first).expect("read first");
        let second_bytes = std::fs::read(&second).expect("read second");
        assert_eq!(first_bytes, second_bytes);
        assert!(!first_bytes.is_empty());
    }

    #[test]
    fn pack_output_round_trips_through_extraction() {
        let dir = tempfile::tempdir().expect("tempdir");
        let rootfs = dir.path().join("rootfs");
        build_fixture(&rootfs);
        let archive_path = dir.path().join("image.tar");
        pack_directory(&rootfs, &archive_path).expect("pack");

        let extracted = dir.path().join("extracted");
        let file = std::fs::File::open(&archive_path).expect("open archive");
        tar::Archive::new(file).unpack(&extracted).expect("unpack");

        let app = std::fs::read(extracted.join("bin/app")).expect("read app");
        assert_eq!(app, b"#!/bin/sh\necho hi\n");
        let config = std::fs::read_to_string(extracted.join("etc/config")).expect("read config");
        assert_eq!(config, "key=value\n");
    }

    #[test]
    fn pack_content_change_changes_bytes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let rootfs = dir.path().join("rootfs");
        build_fixture(&rootfs);
        let original = dir.path().join("original.tar");
        pack_directory(&rootfs, &original).expect("pack original");

        std::fs::write(rootfs.join("etc/config"), b"key=other\n").expect("mutate");
        let mutated = dir.path().join("mutated.tar");
        pack_directory(&rootfs, &mutated).expect("pack mutated");

        let original_bytes = std::fs::read(&original).expect("read original");
        let mutated_bytes = std::fs::read(&mutated).expect("read mutated");
        assert_ne!(original_bytes, mutated_bytes);
    }

    #[test]
    fn pack_hashed_digest_matches_written_archive() {
        let dir = tempfile::tempdir().expect("tempdir");
        let rootfs = dir.path().join("rootfs");
        build_fixture(&rootfs);
        let archive_path = dir.path().join("image.tar");

        let digest = pack_directory_hashed(&rootfs, &archive_path).expect("pack");

        let reread = crate::hash::hash_file(&archive_path).expect("hash");
        assert_eq!(digest.as_hex(), reread.as_hex());
    }

    #[test]
    fn pack_missing_source_returns_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let result = pack_directory(
            &dir.path().join("does-not-exist"),
            &dir.path().join("out.tar"),
        );
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn pack_preserves_symlinks_without_following() {
        let dir = tempfile::tempdir().expect("tempdir");
        let rootfs = dir.path().join("rootfs");
        build_fixture(&rootfs);
        std::os::unix::fs::symlink("bin/app", rootfs.join("entry")).expect("symlink");

        let archive_path = dir.path().join("image.tar");
        pack_directory(&rootfs, &archive_path).expect("pack");

        let extracted = dir.path().join("extracted");
        let file = std::fs::File::open(&archive_path).expect("open archive");
        tar::Archive::new(file).unpack(&extracted).expect("unpack");
        let link = std::fs::read_link(extracted.join("entry")).expect("read link");
        assert_eq!(link, PathBuf::from("bin/app"));
    }
}
