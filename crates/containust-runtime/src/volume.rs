//! Volume mount specification validation.
//!
//! Validates host→container bind mounts before spawn so unsafe paths
//! fail closed in the parent process, not only inside `pre_exec`.

use std::path::{Component, Path, PathBuf};

use containust_common::error::{ContainustError, Result};

/// A validated host-to-container bind mount.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeMount {
    /// Absolute host path (canonicalized when it exists).
    pub source: PathBuf,
    /// Absolute container path (no `..` components).
    pub target: PathBuf,
    /// When true, the mount is remounted read-only after bind.
    pub readonly: bool,
}

/// Parses and validates a `source:target[:ro|rw]` volume specification.
///
/// # Errors
///
/// Returns an error for relative paths, path traversal, missing sources,
/// or invalid mode flags.
pub fn parse_and_validate_volume(spec: &str) -> Result<VolumeMount> {
    let mut parts = spec.split(':');
    let source = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();
    let mode = parts.next();
    if source.is_empty() || target.is_empty() || parts.next().is_some() {
        return Err(ContainustError::Config {
            message: format!("invalid volume specification: {spec}"),
        });
    }
    if mode.is_some_and(|value| value != "ro" && value != "rw") {
        return Err(ContainustError::Config {
            message: format!("unsafe volume specification (bad mode): {spec}"),
        });
    }
    let source_path = Path::new(source);
    let target_path = Path::new(target);
    if !source_path.is_absolute() || !target_path.is_absolute() {
        return Err(ContainustError::Config {
            message: format!("volume paths must be absolute: {spec}"),
        });
    }
    if has_parent_dir(source_path) || has_parent_dir(target_path) {
        return Err(ContainustError::Config {
            message: format!("volume path must not contain '..': {spec}"),
        });
    }
    let source = if source_path.exists() {
        std::fs::canonicalize(source_path).map_err(|source_err| ContainustError::Io {
            path: source_path.to_path_buf(),
            source: source_err,
        })?
    } else {
        return Err(ContainustError::NotFound {
            kind: "volume source",
            id: source.to_string(),
        });
    };
    Ok(VolumeMount {
        source,
        target: target_path.to_path_buf(),
        readonly: mode == Some("ro"),
    })
}

/// Validates every volume spec in `volumes`.
///
/// # Errors
///
/// Returns the first validation error encountered.
pub fn validate_volumes(volumes: &[String]) -> Result<Vec<VolumeMount>> {
    volumes
        .iter()
        .map(|spec| parse_and_validate_volume(spec))
        .collect()
}

fn has_parent_dir(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::ParentDir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_readonly_volume_succeeds_for_existing_source() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("data");
        std::fs::create_dir_all(&source).expect("mkdir");
        let spec = format!("{}:/app/data:ro", source.display());
        let mount = parse_and_validate_volume(&spec).expect("parse");
        assert!(mount.readonly);
        assert_eq!(mount.target, Path::new("/app/data"));
        assert!(mount.source.is_absolute());
    }

    #[test]
    fn parse_rejects_parent_dir_in_source_or_target() {
        assert!(parse_and_validate_volume("/tmp/../etc:/data").is_err());
        assert!(parse_and_validate_volume("/tmp:/app/../escape").is_err());
    }

    #[test]
    fn parse_rejects_relative_paths_and_missing_source() {
        assert!(parse_and_validate_volume("relative:/data").is_err());
        assert!(parse_and_validate_volume("/no/such/path/ctst:/data").is_err());
    }

    #[test]
    fn validate_volumes_returns_all_parsed_mounts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        std::fs::create_dir_all(&a).expect("mkdir a");
        std::fs::create_dir_all(&b).expect("mkdir b");
        let volumes = vec![
            format!("{}:/a", a.display()),
            format!("{}:/b:ro", b.display()),
        ];
        let mounts = validate_volumes(&volumes).expect("validate");
        assert_eq!(mounts.len(), 2);
        assert!(!mounts[0].readonly);
        assert!(mounts[1].readonly);
    }
}
