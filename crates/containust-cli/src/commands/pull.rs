//! `ctst pull` — Pull an OCI registry image into the local catalog.

use std::path::Path;

use clap::Args;
use containust_image::import::{ImportRequest, import_image};
use containust_image::reference::{ImageReference, ImageScheme};

/// Arguments for the `pull` command.
#[derive(Args, Debug)]
pub struct PullArgs {
    /// Image to pull: `name[:tag]`, `oci://…`, optionally `@sha256:<digest>`.
    pub image: String,

    /// Catalog name to register the image under (defaults to the repository name).
    #[arg(long)]
    pub name: Option<String>,

    /// Path to the .ctst composition file whose project store receives the image.
    #[arg(long, default_value = "containust.ctst")]
    pub file: String,
}

/// Executes the `pull` command.
///
/// Resolves the tag against the registry, verifies every manifest and
/// layer by SHA-256, and registers the image in the project catalog.
/// The resolved digest is printed so compositions can pin it.
///
/// # Errors
///
/// Returns an error when offline mode is active, the reference is not
/// an OCI image, or the pull/import fails.
pub fn execute(args: PullArgs, options: &super::RuntimeOptions) -> anyhow::Result<()> {
    let uri = normalize_oci_uri(&args.image);
    let reference = ImageReference::parse(&uri).map_err(|e| anyhow::anyhow!("{e}"))?;
    if reference.scheme() != ImageScheme::Oci {
        anyhow::bail!(
            "ctst pull only accepts registry images (got {uri}); \
             use `ctst build` for file://, tar://, and preset:// sources"
        );
    }
    let catalog_name = args
        .name
        .clone()
        .unwrap_or_else(|| default_catalog_name(reference.location()));

    println!("Pulling {reference} as '{catalog_name}'...");
    let engine = options.engine_for_project(Path::new(&args.file));
    let request = ImportRequest::new(&catalog_name, options.offline).with_unpinned();
    let entry = import_image(engine.data_dir(), &reference, &request)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let digest = entry.digest.as_deref().unwrap_or_default();
    println!(
        "Pulled {} layer(s), {} bytes.",
        entry.layers.len(),
        entry.size_bytes
    );
    println!("Pinned reference: image://{catalog_name}@sha256:{digest}");
    Ok(())
}

/// Prepends `oci://` to bare `name[:tag]` references.
fn normalize_oci_uri(image: &str) -> String {
    if image.contains("://") {
        image.to_string()
    } else {
        format!("oci://{image}")
    }
}

/// Derives a catalog name from the repository's final path segment.
fn default_catalog_name(location: &str) -> String {
    let repository = location.split(':').next().unwrap_or(location);
    repository
        .rsplit('/')
        .next()
        .unwrap_or(repository)
        .to_string()
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn normalize_bare_name_gains_oci_scheme() {
        assert_eq!(normalize_oci_uri("alpine:3.21"), "oci://alpine:3.21");
    }

    #[test]
    fn normalize_existing_scheme_is_unchanged() {
        assert_eq!(normalize_oci_uri("oci://ghcr.io/a/b"), "oci://ghcr.io/a/b");
        assert_eq!(normalize_oci_uri("file:///rootfs"), "file:///rootfs");
    }

    #[test]
    fn default_catalog_name_uses_last_repository_segment() {
        assert_eq!(default_catalog_name("ghcr.io/org/app:v1"), "app");
        assert_eq!(default_catalog_name("alpine:3.21"), "alpine");
        assert_eq!(default_catalog_name("alpine"), "alpine");
    }

    #[test]
    fn pull_rejects_non_oci_scheme() {
        let args = PullArgs {
            image: "file:///rootfs".into(),
            name: None,
            file: "containust.ctst".into(),
        };
        let error =
            execute(args, &super::super::RuntimeOptions::default()).expect_err("non-oci must fail");
        assert!(error.to_string().contains("registry images"));
    }
}
