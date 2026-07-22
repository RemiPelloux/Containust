//! Negative tests for path-escape rejection during archive extraction.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::Path;

use containust_image::extract::safe_extract_archive;

fn write_tar(path: &Path, entries: &[(&str, &[u8])]) {
    let file = std::fs::File::create(path).expect("create tar");
    let mut builder = tar::Builder::new(file);
    for (name, data) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, name, *data)
            .expect("append");
    }
    builder.finish().expect("finish");
}

/// Builds a tar with an attacker-controlled name that the `tar` crate
/// would refuse to emit through its safe builder APIs.
fn write_raw_named_tar(path: &Path, name: &str, data: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_entry_type(tar::EntryType::Regular);
    let name_bytes = name.as_bytes();
    assert!(name_bytes.len() < 100, "ustar name field is 100 bytes");
    header.as_old_mut().name = [0; 100];
    header.as_old_mut().name[..name_bytes.len()].copy_from_slice(name_bytes);
    header.set_cksum();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(header.as_bytes());
    bytes.extend_from_slice(data);
    let pad = (512 - (data.len() % 512)) % 512;
    bytes.extend(std::iter::repeat_n(0_u8, pad));
    bytes.extend(std::iter::repeat_n(0_u8, 1024));
    std::fs::write(path, bytes).expect("write raw tar");
}

fn write_symlink_tar(path: &Path, name: &str, target: &str) {
    let file = std::fs::File::create(path).expect("create tar");
    let mut builder = tar::Builder::new(file);
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Symlink);
    header.set_size(0);
    header.set_mode(0o777);
    header.set_cksum();
    builder
        .append_link(&mut header, name, target)
        .expect("append link");
    builder.finish().expect("finish");
}

#[test]
fn safe_extract_accepts_normal_relative_entries() {
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = dir.path().join("ok.tar");
    write_tar(&archive, &[("bin/app", b"#!/bin/sh\n")]);
    let target = dir.path().join("out");
    safe_extract_archive(&archive, &target).expect("extract");
    assert_eq!(
        std::fs::read(target.join("bin/app")).expect("read"),
        b"#!/bin/sh\n"
    );
}

#[test]
fn safe_extract_rejects_parent_dir_traversal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = dir.path().join("evil.tar");
    write_raw_named_tar(&archive, "../escape.txt", b"pwned");
    let target = dir.path().join("out");
    let error = safe_extract_archive(&archive, &target).expect_err("must reject");
    assert!(error.to_string().contains("unsafe archive entry"));
    assert!(!dir.path().join("escape.txt").exists());
}

#[test]
fn safe_extract_rejects_absolute_path_entry() {
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = dir.path().join("abs.tar");
    write_raw_named_tar(&archive, "/etc/evil", b"nope");
    let target = dir.path().join("out");
    let error = safe_extract_archive(&archive, &target).expect_err("must reject");
    assert!(error.to_string().contains("unsafe archive entry"));
}

#[test]
fn safe_extract_rejects_symlink_escape() {
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = dir.path().join("link.tar");
    write_symlink_tar(&archive, "escape", "../../outside");
    let target = dir.path().join("out");
    let error = safe_extract_archive(&archive, &target).expect_err("must reject");
    assert!(error.to_string().contains("unsafe archive entry"));
}

#[cfg(unix)]
#[test]
fn safe_extract_rejects_chained_symlink_escape() {
    // Classic tar-slip: subdir/up -> .. ; subdir/up2 -> up/.. ; then write
    // through up2. Lexical depth checks miss this; resolve-under-root must not.
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = dir.path().join("chain.tar");
    let file = std::fs::File::create(&archive).expect("create tar");
    let mut builder = tar::Builder::new(file);

    let mut dir_header = tar::Header::new_gnu();
    dir_header.set_entry_type(tar::EntryType::Directory);
    dir_header.set_size(0);
    dir_header.set_mode(0o755);
    dir_header.set_cksum();
    builder
        .append_data(&mut dir_header, "subdir", std::io::empty())
        .expect("dir");

    let mut up = tar::Header::new_gnu();
    up.set_entry_type(tar::EntryType::Symlink);
    up.set_size(0);
    up.set_mode(0o777);
    up.set_cksum();
    builder.append_link(&mut up, "subdir/up", "..").expect("up");

    let mut up2 = tar::Header::new_gnu();
    up2.set_entry_type(tar::EntryType::Symlink);
    up2.set_size(0);
    up2.set_mode(0o777);
    up2.set_cksum();
    builder
        .append_link(&mut up2, "subdir/up2", "up/..")
        .expect("up2");
    builder.finish().expect("finish");

    let target = dir.path().join("out");
    let error = safe_extract_archive(&archive, &target).expect_err("chain must fail");
    assert!(
        error.to_string().contains("unsafe archive entry"),
        "unexpected error: {error}"
    );
    assert!(
        !target.exists(),
        "failed extract must wipe the target so no escape chain remains"
    );
}
