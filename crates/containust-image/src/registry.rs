//! Local image catalog management.
//!
//! Maintains an index of available images and their layer compositions.

use std::path::{Path, PathBuf};

use containust_common::error::{ContainustError, Result};
use containust_common::types::ImageId;
use serde::{Deserialize, Serialize};

/// Entry in the local image catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageEntry {
    /// Unique identifier for this image.
    pub id: ImageId,
    /// Human-readable name/tag.
    pub name: String,
    /// Source URI this image was loaded from.
    pub source: String,
    /// Ordered list of layer hashes (bottom to top).
    pub layers: Vec<String>,
    /// Total size in bytes.
    pub size_bytes: u64,
    /// Creation timestamp (ISO-8601).
    pub created_at: String,
}

/// Image catalog backed by a JSON file.
#[derive(Debug)]
pub struct ImageCatalog {
    catalog_path: PathBuf,
}

impl ImageCatalog {
    /// Opens or creates an image catalog at the given directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the catalog directory cannot be created.
    pub fn open(data_dir: &Path) -> Result<Self> {
        let catalog_path = data_dir.join("images").join("catalog.json");
        if let Some(parent) = catalog_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ContainustError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        Ok(Self { catalog_path })
    }

    /// Lists all images in the catalog.
    ///
    /// # Errors
    ///
    /// Returns an error if the catalog file cannot be read or parsed.
    pub fn list(&self) -> Result<Vec<ImageEntry>> {
        if !self.catalog_path.exists() {
            return Ok(Vec::new());
        }
        let content =
            std::fs::read_to_string(&self.catalog_path).map_err(|e| ContainustError::Io {
                path: self.catalog_path.clone(),
                source: e,
            })?;
        let entries: Vec<ImageEntry> = serde_json::from_str(&content)?;
        Ok(entries)
    }

    /// Registers a new image in the catalog.
    ///
    /// # Errors
    ///
    /// Returns an error if the catalog cannot be read or written.
    pub fn register(&self, entry: ImageEntry) -> Result<()> {
        let mut entries = self.list()?;
        entries.push(entry);
        self.write_entries(&entries)
    }

    /// Removes an image by ID.
    ///
    /// # Errors
    ///
    /// Returns `ContainustError::NotFound` if no image with the given ID exists.
    pub fn remove(&self, id: &ImageId) -> Result<()> {
        let mut entries = self.list()?;
        let before = entries.len();
        entries.retain(|e| e.id.as_str() != id.as_str());
        if entries.len() == before {
            return Err(ContainustError::NotFound {
                kind: "image",
                id: id.to_string(),
            });
        }
        self.write_entries(&entries)
    }

    fn write_entries(&self, entries: &[ImageEntry]) -> Result<()> {
        let json = serde_json::to_string_pretty(entries)?;
        std::fs::write(&self.catalog_path, json).map_err(|e| ContainustError::Io {
            path: self.catalog_path.clone(),
            source: e,
        })?;
        Ok(())
    }
}

/// Lists all images in the default catalog location.
///
/// # Errors
///
/// Returns an error if the catalog cannot be opened or read.
pub fn list_images() -> Result<Vec<ImageEntry>> {
    let catalog = ImageCatalog::open(Path::new(containust_common::constants::DEFAULT_DATA_DIR))?;
    catalog.list()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: &str, name: &str) -> ImageEntry {
        ImageEntry {
            id: ImageId::new(id),
            name: name.into(),
            source: format!("file:///opt/images/{name}"),
            layers: vec!["abc123".into()],
            size_bytes: 1024,
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn catalog_empty_on_first_open() {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let catalog = ImageCatalog::open(dir.path()).expect("open failed");
        assert!(catalog.list().expect("list failed").is_empty());
    }

    #[test]
    fn catalog_register_and_list_single_image() {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let catalog = ImageCatalog::open(dir.path()).expect("open failed");

        catalog
            .register(make_entry("img-1", "alpine"))
            .expect("register failed");

        let entries = catalog.list().expect("list failed");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "alpine");
        assert_eq!(entries[0].size_bytes, 1024);
    }

    #[test]
    fn catalog_remove_existing_image() {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let catalog = ImageCatalog::open(dir.path()).expect("open failed");

        catalog
            .register(make_entry("img-1", "alpine"))
            .expect("register failed");
        catalog
            .remove(&ImageId::new("img-1"))
            .expect("remove failed");

        assert!(catalog.list().expect("list failed").is_empty());
    }

    #[test]
    fn catalog_remove_nonexistent_returns_error() {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let catalog = ImageCatalog::open(dir.path()).expect("open failed");
        assert!(catalog.remove(&ImageId::new("nonexistent")).is_err());
    }

    #[test]
    fn catalog_register_multiple_images() {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let catalog = ImageCatalog::open(dir.path()).expect("open failed");

        catalog
            .register(make_entry("img-1", "alpine"))
            .expect("register failed");
        catalog
            .register(make_entry("img-2", "debian"))
            .expect("register failed");

        let entries = catalog.list().expect("list failed");
        assert_eq!(entries.len(), 2);
    }
}
