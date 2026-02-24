//! IMPORT resolution from files and network sources.
//!
//! Resolves `IMPORT` declarations to their source `.ctst` files,
//! supporting local file paths. Remote URLs are not yet supported.

use std::path::{Path, PathBuf};

use containust_common::error::{ContainustError, Result};

use crate::parser::ast::CompositionFile;

/// Resolves an import declaration and parses the referenced file.
///
/// Absolute paths are used as-is; relative paths are resolved
/// against `base_dir`.
///
/// # Errors
///
/// Returns an error if the source cannot be found, read, or parsed.
pub fn resolve_import(source: &str, base_dir: &Path) -> Result<CompositionFile> {
    tracing::info!(source = source, "resolving import");

    let path = if source.starts_with('/') {
        PathBuf::from(source)
    } else {
        base_dir.join(source)
    };

    if !path.exists() {
        return Err(ContainustError::NotFound {
            kind: "import file",
            id: source.to_string(),
        });
    }

    let content = std::fs::read_to_string(&path).map_err(|e| ContainustError::Io {
        path: path.clone(),
        source: e,
    })?;

    crate::parser::parse_ctst(&content)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn resolve_import_from_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file_path = dir.path().join("base.ctst");
        let mut f = std::fs::File::create(&file_path).expect("create");
        write!(
            f,
            r#"COMPONENT db {{
    image = "postgres:15"
    port = 5432
}}"#
        )
        .expect("write");

        let result = resolve_import("base.ctst", dir.path());
        assert!(result.is_ok(), "error: {result:?}");
        let comp = &result.expect("parsed").components[0];
        assert_eq!(comp.name, "db");
        assert_eq!(comp.port, Some(5432));
    }

    #[test]
    fn resolve_import_absolute_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file_path = dir.path().join("abs.ctst");
        let mut f = std::fs::File::create(&file_path).expect("create");
        write!(f, r#"COMPONENT svc {{ image = "img" }}"#).expect("write");

        let abs = file_path.to_str().expect("path str");
        let result = resolve_import(abs, Path::new("/nonexistent"));
        assert!(result.is_ok(), "error: {result:?}");
    }

    #[test]
    fn resolve_import_missing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let result = resolve_import("nonexistent.ctst", dir.path());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("not found"), "got: {msg}");
    }

    #[test]
    fn resolve_import_nested_dir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sub = dir.path().join("templates");
        std::fs::create_dir(&sub).expect("mkdir");
        let file_path = sub.join("pg.ctst");
        let mut f = std::fs::File::create(&file_path).expect("create");
        write!(f, r#"COMPONENT pg {{ image = "postgres" }}"#).expect("write");

        let result = resolve_import("templates/pg.ctst", dir.path());
        assert!(result.is_ok(), "error: {result:?}");
    }
}
