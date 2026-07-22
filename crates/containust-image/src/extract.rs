//! Safe archive extraction that fails closed on escape attempts.
//!
//! Rejects absolute paths, `..` components, hard links, device nodes,
//! and symlink targets that resolve outside the extraction root —
//! including chained-symlink escapes.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use containust_common::error::{ContainustError, Result};

use crate::path_confine::{assert_dest_confined, ensure_symlink_confined};

const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

/// Extracts a tar (optionally gzip-compressed) archive into `target`.
///
/// On failure the target directory is removed so a partial extract
/// cannot leave a planted symlink chain behind.
///
/// # Errors
///
/// Returns an error if the archive cannot be read, contains an unsafe
/// entry, or a filesystem write fails.
pub fn safe_extract_archive(archive_path: &Path, target: &Path) -> Result<()> {
    std::fs::create_dir_all(target).map_err(|source| ContainustError::Io {
        path: target.to_path_buf(),
        source,
    })?;
    match extract_into(archive_path, target) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = std::fs::remove_dir_all(target);
            Err(error)
        }
    }
}

fn extract_into(archive_path: &Path, target: &Path) -> Result<()> {
    let file = std::fs::File::open(archive_path).map_err(|source| ContainustError::Io {
        path: archive_path.to_path_buf(),
        source,
    })?;
    let mut peek = [0_u8; 2];
    let mut reader = std::io::BufReader::new(file);
    let gzip = reader
        .read(&mut peek)
        .map_err(|source| ContainustError::Io {
            path: archive_path.to_path_buf(),
            source,
        })?
        == 2
        && peek == GZIP_MAGIC;
    let file = std::fs::File::open(archive_path).map_err(|source| ContainustError::Io {
        path: archive_path.to_path_buf(),
        source,
    })?;
    if gzip {
        unpack_entries(
            tar::Archive::new(flate2::read::GzDecoder::new(file)),
            target,
        )
    } else {
        unpack_entries(tar::Archive::new(file), target)
    }
}

fn unpack_entries<R: Read>(mut archive: tar::Archive<R>, target: &Path) -> Result<()> {
    let entries = archive.entries().map_err(|source| ContainustError::Io {
        path: target.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let mut entry = entry.map_err(|source| ContainustError::Io {
            path: target.to_path_buf(),
            source,
        })?;
        unpack_one(&mut entry, target)?;
    }
    Ok(())
}

fn unpack_one<R: Read>(entry: &mut tar::Entry<'_, R>, target: &Path) -> Result<()> {
    let header = entry.header().clone();
    let entry_path = entry.path().map_err(|source| ContainustError::Io {
        path: target.to_path_buf(),
        source,
    })?;
    let relative = sanitize_entry_path(&entry_path)?;
    let dest = target.join(&relative);
    match header.entry_type() {
        tar::EntryType::Regular | tar::EntryType::Continuous => {
            write_regular_file(entry, target, &dest)?;
        }
        tar::EntryType::Directory => {
            assert_dest_confined(target, &dest)?;
            std::fs::create_dir_all(&dest).map_err(|source| ContainustError::Io {
                path: dest.clone(),
                source,
            })?;
        }
        tar::EntryType::Symlink => {
            let link = entry.link_name().map_err(|source| ContainustError::Io {
                path: dest.clone(),
                source,
            })?;
            let Some(link) = link else {
                return Err(unsafe_entry("symlink missing target", &entry_path));
            };
            ensure_symlink_confined(target, &dest, &link)?;
            create_symlink(&link, target, &dest)?;
        }
        tar::EntryType::Link
        | tar::EntryType::Char
        | tar::EntryType::Block
        | tar::EntryType::Fifo => {
            return Err(unsafe_entry(
                &format!("unsupported entry type {:?}", header.entry_type()),
                &entry_path,
            ));
        }
        other => {
            return Err(unsafe_entry(
                &format!("unsupported entry type {other:?}"),
                &entry_path,
            ));
        }
    }
    Ok(())
}

/// Rejects absolute paths and any `..` component.
pub(crate) fn sanitize_entry_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Err(unsafe_entry("absolute path", path));
    }
    let mut sanitized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => sanitized.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(unsafe_entry("path traversal (..)", path));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(unsafe_entry("absolute path", path));
            }
        }
    }
    if sanitized.as_os_str().is_empty() {
        return Err(unsafe_entry("empty path", path));
    }
    Ok(sanitized)
}

fn write_regular_file<R: Read>(
    entry: &mut tar::Entry<'_, R>,
    root: &Path,
    dest: &Path,
) -> Result<()> {
    if let Some(parent) = dest.parent() {
        assert_dest_confined(root, parent)?;
        std::fs::create_dir_all(parent).map_err(|source| ContainustError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    assert_dest_confined(root, dest)?;
    let mut out = std::fs::File::create(dest).map_err(|source| ContainustError::Io {
        path: dest.to_path_buf(),
        source,
    })?;
    let _ = std::io::copy(entry, &mut out).map_err(|source| ContainustError::Io {
        path: dest.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn create_symlink(link: &Path, root: &Path, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        assert_dest_confined(root, parent)?;
        std::fs::create_dir_all(parent).map_err(|source| ContainustError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(link, dest).map_err(|source| ContainustError::Io {
            path: dest.to_path_buf(),
            source,
        })
    }
    #[cfg(not(unix))]
    {
        let _ = (link, dest);
        Err(ContainustError::Config {
            message: "symlink extraction requires a Unix host".into(),
        })
    }
}

fn unsafe_entry(reason: &str, path: &Path) -> ContainustError {
    ContainustError::Config {
        message: format!(
            "unsafe archive entry rejected ({reason}): {}",
            path.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_entry_path_allows_nested_relative() {
        let path = sanitize_entry_path(Path::new("usr/bin/sh")).expect("sanitize");
        assert_eq!(path, PathBuf::from("usr/bin/sh"));
    }

    #[test]
    fn sanitize_entry_path_rejects_parent_dir() {
        assert!(sanitize_entry_path(Path::new("../escape")).is_err());
    }
}
