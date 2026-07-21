//! Local image catalog management.
//!
//! Maintains a JSON index of imported images. The catalog is guarded by
//! a cross-process file lock and written atomically (temporary file +
//! rename) so competing CLI processes cannot corrupt it. Registrations
//! are deduplicated by name and every referenced layer must exist in
//! the content-addressed store before an entry is accepted.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use containust_common::error::{ContainustError, Result};
use containust_common::types::ImageId;
use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::storage::StorageBackend;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

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
    /// SHA-256 digest of the image content, when known.
    #[serde(default)]
    pub digest: Option<String>,
    /// Version of the tool that imported this image.
    #[serde(default)]
    pub tool_version: String,
}

/// Image catalog backed by a locked, atomically written JSON file.
#[derive(Debug)]
pub struct ImageCatalog {
    data_dir: PathBuf,
    catalog_path: PathBuf,
    lock_path: PathBuf,
}

impl ImageCatalog {
    /// Opens or creates an image catalog under the given data directory.
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
        let lock_path = PathBuf::from(format!("{}.lock", catalog_path.display()));
        Ok(Self {
            data_dir: data_dir.to_path_buf(),
            catalog_path,
            lock_path,
        })
    }

    /// Lists all images under a shared catalog lock.
    ///
    /// # Errors
    ///
    /// Returns an error if the catalog file cannot be read or parsed.
    pub fn list(&self) -> Result<Vec<ImageEntry>> {
        let _guard = self.lock(false)?;
        self.read_entries()
    }

    /// Finds an image by name or ID.
    ///
    /// # Errors
    ///
    /// Returns `ContainustError::NotFound` if no matching image exists.
    pub fn find(&self, target: &str) -> Result<ImageEntry> {
        self.list()?
            .into_iter()
            .find(|entry| entry.name == target || entry.id.as_str() == target)
            .ok_or_else(|| ContainustError::NotFound {
                kind: "image",
                id: target.to_string(),
            })
    }

    /// Registers an image, replacing any previous entry with the same name.
    ///
    /// Every referenced layer must already exist in the local layer
    /// store; a catalog must never point at content that is not present.
    ///
    /// # Errors
    ///
    /// Returns an error if a referenced layer is missing or the catalog
    /// cannot be read or written.
    pub fn register(&self, entry: ImageEntry) -> Result<()> {
        self.validate_layers(&entry)?;
        let _guard = self.lock(true)?;
        let mut entries = self.read_entries()?;
        entries.retain(|existing| {
            let duplicate = existing.name == entry.name;
            if duplicate {
                tracing::info!(name = %entry.name, "replacing existing catalog entry");
            }
            !duplicate
        });
        entries.push(entry);
        self.write_entries(&entries)
    }

    /// Removes an image by ID under an exclusive lock.
    ///
    /// # Errors
    ///
    /// Returns `ContainustError::NotFound` if no image with the given ID exists.
    pub fn remove(&self, id: &ImageId) -> Result<()> {
        let _guard = self.lock(true)?;
        let mut entries = self.read_entries()?;
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

    fn validate_layers(&self, entry: &ImageEntry) -> Result<()> {
        let store = StorageBackend::open(self.data_dir.clone())?;
        for layer in &entry.layers {
            if !store.has_layer(layer) {
                return Err(ContainustError::NotFound {
                    kind: "image layer",
                    id: format!("{layer} (referenced by image '{}')", entry.name),
                });
            }
        }
        Ok(())
    }

    fn read_entries(&self) -> Result<Vec<ImageEntry>> {
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

    fn write_entries(&self, entries: &[ImageEntry]) -> Result<()> {
        let json = serde_json::to_vec_pretty(entries)?;
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp_path = self
            .catalog_path
            .with_extension(format!("tmp-{}-{counter}", std::process::id()));
        let write_result = write_and_replace(&temp_path, &self.catalog_path, &json);
        if write_result.is_err() {
            let _ = std::fs::remove_file(&temp_path);
        }
        write_result
    }

    fn lock(&self, exclusive: bool) -> Result<CatalogLock> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&self.lock_path)
            .map_err(|source| ContainustError::Io {
                path: self.lock_path.clone(),
                source,
            })?;
        let lock_result = if exclusive {
            FileExt::lock_exclusive(&file)
        } else {
            FileExt::lock_shared(&file)
        };
        lock_result.map_err(|source| ContainustError::Io {
            path: self.lock_path.clone(),
            source,
        })?;
        Ok(CatalogLock { file })
    }
}

struct CatalogLock {
    file: std::fs::File,
}

impl Drop for CatalogLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

fn write_and_replace(temp_path: &Path, path: &Path, json: &[u8]) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(temp_path)
        .map_err(|source| ContainustError::Io {
            path: temp_path.to_path_buf(),
            source,
        })?;
    file.write_all(json).map_err(|source| ContainustError::Io {
        path: temp_path.to_path_buf(),
        source,
    })?;
    file.sync_all().map_err(|source| ContainustError::Io {
        path: temp_path.to_path_buf(),
        source,
    })?;
    drop(file);
    atomic_replace(temp_path, path)
}

#[cfg(not(windows))]
fn atomic_replace(temp_path: &Path, path: &Path) -> Result<()> {
    std::fs::rename(temp_path, path).map_err(|source| ContainustError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(windows)]
fn atomic_replace(temp_path: &Path, path: &Path) -> Result<()> {
    // Windows cannot rename over an existing file. The remove+rename
    // pair runs under the exclusive catalog lock, so no concurrent
    // process can observe the gap; a crash in between leaves the fully
    // written temporary file for manual recovery.
    if path.exists() {
        std::fs::remove_file(path).map_err(|source| ContainustError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }
    std::fs::rename(temp_path, path).map_err(|source| ContainustError::Io {
        path: path.to_path_buf(),
        source,
    })
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

    fn store_layer(data_dir: &Path, hash: &str) {
        let store = StorageBackend::open(data_dir.to_path_buf()).expect("open store");
        let staged = store.staging_path();
        std::fs::write(&staged, b"layer").expect("write staged");
        store.commit_layer(&staged, hash).expect("commit layer");
    }

    fn make_entry(id: &str, name: &str, layers: Vec<String>) -> ImageEntry {
        ImageEntry {
            id: ImageId::new(id),
            name: name.into(),
            source: format!("file:///opt/images/{name}"),
            layers,
            size_bytes: 1024,
            created_at: "2026-01-01T00:00:00Z".into(),
            digest: Some("a".repeat(64)),
            tool_version: "0.4.0".into(),
        }
    }

    #[test]
    fn catalog_empty_on_first_open() {
        let dir = tempfile::tempdir().expect("tempdir");
        let catalog = ImageCatalog::open(dir.path()).expect("open failed");
        assert!(catalog.list().expect("list failed").is_empty());
    }

    #[test]
    fn catalog_register_and_list_single_image() {
        let dir = tempfile::tempdir().expect("tempdir");
        store_layer(dir.path(), "abc123");
        let catalog = ImageCatalog::open(dir.path()).expect("open failed");

        catalog
            .register(make_entry("img-1", "alpine", vec!["abc123".into()]))
            .expect("register failed");

        let entries = catalog.list().expect("list failed");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "alpine");
        assert_eq!(entries[0].tool_version, "0.4.0");
    }

    #[test]
    fn catalog_register_missing_layer_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let catalog = ImageCatalog::open(dir.path()).expect("open failed");
        let result = catalog.register(make_entry("img-1", "alpine", vec!["missing".into()]));
        assert!(result.is_err());
        assert!(catalog.list().expect("list").is_empty());
    }

    #[test]
    fn catalog_register_same_name_replaces_entry() {
        let dir = tempfile::tempdir().expect("tempdir");
        store_layer(dir.path(), "layer-a");
        store_layer(dir.path(), "layer-b");
        let catalog = ImageCatalog::open(dir.path()).expect("open failed");

        catalog
            .register(make_entry("img-1", "web", vec!["layer-a".into()]))
            .expect("first register");
        catalog
            .register(make_entry("img-2", "web", vec!["layer-b".into()]))
            .expect("second register");

        let entries = catalog.list().expect("list");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id.as_str(), "img-2");
        assert_eq!(entries[0].layers, vec!["layer-b".to_string()]);
    }

    #[test]
    fn catalog_find_by_name_and_id() {
        let dir = tempfile::tempdir().expect("tempdir");
        let catalog = ImageCatalog::open(dir.path()).expect("open failed");
        catalog
            .register(make_entry("img-1", "alpine", Vec::new()))
            .expect("register");

        assert_eq!(
            catalog.find("alpine").expect("by name").id.as_str(),
            "img-1"
        );
        assert_eq!(catalog.find("img-1").expect("by id").name, "alpine");
        assert!(catalog.find("missing").is_err());
    }

    #[test]
    fn catalog_remove_existing_image() {
        let dir = tempfile::tempdir().expect("tempdir");
        let catalog = ImageCatalog::open(dir.path()).expect("open failed");
        catalog
            .register(make_entry("img-1", "alpine", Vec::new()))
            .expect("register failed");
        catalog
            .remove(&ImageId::new("img-1"))
            .expect("remove failed");
        assert!(catalog.list().expect("list failed").is_empty());
    }

    #[test]
    fn catalog_remove_nonexistent_returns_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let catalog = ImageCatalog::open(dir.path()).expect("open failed");
        assert!(catalog.remove(&ImageId::new("nonexistent")).is_err());
    }

    #[test]
    fn catalog_reads_legacy_entries_without_new_fields() {
        let dir = tempfile::tempdir().expect("tempdir");
        let catalog_dir = dir.path().join("images");
        std::fs::create_dir_all(&catalog_dir).expect("mkdir");
        std::fs::write(
            catalog_dir.join("catalog.json"),
            r#"[{"id":"legacy","name":"old","source":"file:///old",
                 "layers":[],"size_bytes":10,"created_at":"2026-01-01T00:00:00Z"}]"#,
        )
        .expect("write legacy catalog");

        let catalog = ImageCatalog::open(dir.path()).expect("open failed");
        let entries = catalog.list().expect("list failed");
        assert_eq!(entries.len(), 1);
        assert!(entries[0].digest.is_none());
        assert!(entries[0].tool_version.is_empty());
    }

    #[test]
    fn catalog_concurrent_registrations_do_not_lose_entries() {
        let dir = tempfile::tempdir().expect("tempdir");
        let data_dir = dir.path().to_path_buf();
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let data_dir = data_dir.clone();
                std::thread::spawn(move || {
                    let catalog = ImageCatalog::open(&data_dir).expect("open");
                    catalog
                        .register(make_entry(
                            &format!("img-{i}"),
                            &format!("image-{i}"),
                            Vec::new(),
                        ))
                        .expect("register");
                })
            })
            .collect();
        for handle in handles {
            handle.join().expect("thread join");
        }

        let catalog = ImageCatalog::open(&data_dir).expect("open");
        assert_eq!(catalog.list().expect("list").len(), 8);
    }
}
