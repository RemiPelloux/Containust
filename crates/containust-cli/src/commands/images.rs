//! `ctst images` — Manage the local image catalog.

use clap::Args;
use containust_common::types::ImageId;
use containust_image::registry::ImageCatalog;

use crate::output;

/// Arguments for the `images` command.
#[derive(Args, Debug)]
pub struct ImagesArgs {
    /// List all images.
    #[arg(short, long)]
    pub list: bool,

    /// Remove an image by ID.
    #[arg(long)]
    pub remove: Option<String>,
}

/// Executes the `images` command.
///
/// Lists or removes images from the local catalog.
///
/// # Errors
///
/// Returns an error if catalog operations fail.
pub fn execute(args: ImagesArgs) -> anyhow::Result<()> {
    let data_dir = std::path::Path::new(containust_common::constants::DEFAULT_DATA_DIR);
    let catalog = ImageCatalog::open(data_dir).map_err(|e| anyhow::anyhow!("{e}"))?;

    if let Some(ref id) = args.remove {
        catalog
            .remove(&ImageId::new(id))
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        println!("Removed image: {id}");
        return Ok(());
    }

    let images = catalog.list().map_err(|e| anyhow::anyhow!("{e}"))?;

    if images.is_empty() {
        println!("No images found.");
        return Ok(());
    }

    println!(
        "{:<40} {:<20} {:<10} {:<15}",
        "IMAGE ID", "NAME", "LAYERS", "SIZE"
    );
    for img in &images {
        println!(
            "{:<40} {:<20} {:<10} {:<15}",
            img.id,
            img.name,
            img.layers.len(),
            output::format_bytes(img.size_bytes)
        );
    }

    Ok(())
}
