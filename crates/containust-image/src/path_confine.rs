//! Path confinement helpers for archive extraction and rootfs copies.
//!
//! Resolves relative paths while following already-extracted symlinks,
//! rejecting any walk that leaves `root`. This closes chained-symlink
//! escapes that pure lexical `..` counting misses.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use containust_common::error::{ContainustError, Result};

/// Maximum symlink hops when resolving under a root (loop guard).
const MAX_SYMLINK_DEPTH: usize = 40;

/// Ensures `link_target` (relative to `link_location`'s parent) resolves
/// under `root`, following already-created symlinks inside the tree.
///
/// # Errors
///
/// Returns an error when the target is absolute, escapes `root`, or
/// exceeds the symlink hop limit.
pub fn ensure_symlink_confined(
    root: &Path,
    link_location: &Path,
    link_target: &Path,
) -> Result<()> {
    if link_target.is_absolute() {
        return Err(escape_error("absolute symlink target", link_target));
    }
    let parent = link_location.parent().unwrap_or(root);
    let _ = resolve_under_root(root, parent, link_target)?;
    Ok(())
}

/// Resolves `relative` starting at `base`, staying inside `root`.
///
/// Existing symlink components are followed (with a hop limit). Missing
/// final components are accepted as long as every prefix stays confined.
///
/// # Errors
///
/// Returns an error on escape, absolute components, or symlink loops.
pub fn resolve_under_root(root: &Path, base: &Path, relative: &Path) -> Result<PathBuf> {
    let mut state = ResolveState {
        root: root.to_path_buf(),
        current: normalize_under_root(root, base)?,
        hops: 0,
        seen: HashSet::new(),
    };
    state.apply(relative)?;
    normalize_under_root(root, &state.current)
}

/// Walks each component of `dest` relative to `root` and rejects the
/// path when an intermediate symlink resolves outside `root`.
///
/// # Errors
///
/// Returns an error when `dest` is not under `root` or a symlink escape
/// is detected.
pub fn assert_dest_confined(root: &Path, dest: &Path) -> Result<()> {
    let relative = dest
        .strip_prefix(root)
        .map_err(|_| escape_error("destination escapes extraction root", dest))?;
    let _ = resolve_under_root(root, root, relative)?;
    Ok(())
}

struct ResolveState {
    root: PathBuf,
    current: PathBuf,
    hops: usize,
    seen: HashSet<PathBuf>,
}

impl ResolveState {
    fn apply(&mut self, relative: &Path) -> Result<()> {
        for component in relative.components() {
            match component {
                Component::Normal(part) => self.step_normal(part)?,
                Component::CurDir => {}
                Component::ParentDir => self.step_parent(relative)?,
                Component::RootDir | Component::Prefix(_) => {
                    return Err(escape_error("absolute path", relative));
                }
            }
        }
        Ok(())
    }

    fn step_normal(&mut self, part: &std::ffi::OsStr) -> Result<()> {
        let next = self.current.join(part);
        if is_symlink(&next) {
            return self.follow(&next);
        }
        self.current = normalize_under_root(&self.root, &next)?;
        Ok(())
    }

    fn step_parent(&mut self, relative: &Path) -> Result<()> {
        if self.current.as_path() == self.root.as_path() {
            return Err(escape_error("path traversal (..)", relative));
        }
        self.current = self
            .current
            .parent()
            .map_or_else(|| self.root.clone(), Path::to_path_buf);
        self.current = normalize_under_root(&self.root, &self.current)?;
        Ok(())
    }

    fn follow(&mut self, link_path: &Path) -> Result<()> {
        self.hops += 1;
        if self.hops > MAX_SYMLINK_DEPTH {
            return Err(escape_error("symlink hop limit exceeded", link_path));
        }
        if !self.seen.insert(link_path.to_path_buf()) {
            return Err(escape_error("symlink loop", link_path));
        }
        let target = std::fs::read_link(link_path).map_err(|source| ContainustError::Io {
            path: link_path.to_path_buf(),
            source,
        })?;
        if target.is_absolute() {
            return Err(escape_error("absolute symlink target", &target));
        }
        self.current = link_path
            .parent()
            .map_or_else(|| self.root.clone(), Path::to_path_buf);
        self.apply(&target)
    }
}

fn normalize_under_root(root: &Path, path: &Path) -> Result<PathBuf> {
    // Lexical only — do not canonicalize. On macOS `/var` vs `/private/var`
    // would otherwise make in-tree paths look like escapes.
    let relative = if let Ok(stripped) = path.strip_prefix(root) {
        stripped.to_path_buf()
    } else if path.is_absolute() {
        return Err(escape_error("path escapes root", path));
    } else {
        path.to_path_buf()
    };
    let mut out = root.to_path_buf();
    for component in relative.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if out.as_path() == root {
                    return Err(escape_error("path traversal (..)", path));
                }
                let _ = out.pop();
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(escape_error("absolute path", path));
            }
        }
    }
    if !out.starts_with(root) {
        return Err(escape_error("path escapes root", path));
    }
    Ok(out)
}

fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

fn escape_error(reason: &str, path: &Path) -> ContainustError {
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
    fn resolve_under_root_accepts_plain_relative() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let resolved = resolve_under_root(root, root, Path::new("bin/app")).expect("resolve");
        assert_eq!(resolved, root.join("bin/app"));
    }

    #[cfg(unix)]
    #[test]
    fn ensure_symlink_confined_rejects_chain_escape() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let sub = root.join("subdir");
        std::fs::create_dir_all(&sub).expect("mkdir");
        std::os::unix::fs::symlink("..", sub.join("up")).expect("link up");
        let err = ensure_symlink_confined(root, &sub.join("up2"), Path::new("up/.."))
            .expect_err("chain must fail");
        assert!(err.to_string().contains("unsafe archive entry"));
    }
}
