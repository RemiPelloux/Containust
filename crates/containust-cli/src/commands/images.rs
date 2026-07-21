//! `ctst images` — Manage the local image catalog and list presets.

use clap::Args;
use containust_common::types::ImageId;
use containust_image::preset::list_presets;
use containust_image::registry::ImageCatalog;

use crate::output;

/// Arguments for the `images` command.
#[derive(Args, Debug)]
pub struct ImagesArgs {
    /// List all images.
    #[arg(short, long)]
    pub list: bool,

    /// List curated `preset://` images available for this host.
    #[arg(long)]
    pub presets: bool,

    /// Remove an image by ID.
    #[arg(long)]
    pub remove: Option<String>,
}

/// Executes the `images` command.
///
/// Lists or removes images from the local catalog, or prints the curated
/// preset catalog for the host architecture.
///
/// # Errors
///
/// Returns an error if catalog operations fail.
pub fn execute(args: ImagesArgs, options: &super::RuntimeOptions) -> anyhow::Result<()> {
    if args.presets {
        print_presets();
        return Ok(());
    }

    let catalog =
        ImageCatalog::open(options.engine().data_dir()).map_err(|e| anyhow::anyhow!("{e}"))?;

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
        println!("Hint: import with `ctst build`, or try `ctst images --presets`.");
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

fn print_presets() {
    let presets = list_presets();
    if presets.is_empty() {
        println!("No curated presets for this architecture.");
        return;
    }
    println!("PRESET           VERSION    ARCH       DESCRIPTION");
    for preset in &presets {
        let label = format!("preset://{}", preset.name);
        println!(
            "{label:<16} {:<10} {:<10} {}",
            preset.version, preset.arch, preset.description
        );
    }
    println!();
    println!("Example: image = \"preset://alpine\" or \"preset://alpine:3.21\"");
    println!(
        "First `ctst build` downloads and pins the archive; later `--offline` uses the cache."
    );
}
