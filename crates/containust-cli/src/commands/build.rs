//! `ctst build` — Import composition images into the local catalog.

use std::path::Path;

use clap::Args;
use containust_image::import::{ImportRequest, import_image};
use containust_image::preset::resolve_preset;
use containust_image::reference::{ImageReference, ImageScheme};

/// Arguments for the `build` command.
#[derive(Args, Debug)]
pub struct BuildArgs {
    /// Path to the .ctst composition file.
    #[arg(default_value = "containust.ctst")]
    pub file: String,

    /// Plan the import without writing layers or catalog entries.
    #[arg(long)]
    pub dry_run: bool,
}

/// Executes the `build` command.
///
/// Parses the `.ctst` file, validates it, and imports every declared
/// image source into the project's content-addressed catalog. With
/// `--dry-run`, only the planned imports are displayed.
///
/// # Errors
///
/// Returns an error if parsing, validation, or an image import fails.
pub fn execute(args: BuildArgs, options: &super::RuntimeOptions) -> anyhow::Result<()> {
    tracing::info!(file = %args.file, dry_run = args.dry_run, "building from .ctst file");

    let content = std::fs::read_to_string(&args.file)?;
    let composition =
        containust_compose::parser::parse_ctst(&content).map_err(|e| anyhow::anyhow!("{e}"))?;
    if options.offline {
        containust_compose::validate_offline(&composition).map_err(|e| anyhow::anyhow!("{e}"))?;
    }

    println!(
        "Parsed {} components, {} connections",
        composition.components.len(),
        composition.connections.len()
    );

    let engine = options.engine_for_project(Path::new(&args.file));
    let mut imported = 0_usize;
    for component in &composition.components {
        let Some(image) = component.image.as_deref() else {
            continue;
        };
        let reference = ImageReference::parse(image).map_err(|e| anyhow::anyhow!("{e}"))?;
        imported += usize::from(build_component(
            &BuildContext {
                data_dir: engine.data_dir(),
                offline: options.offline,
                dry_run: args.dry_run,
            },
            &component.name,
            &reference,
        )?);
    }

    if args.dry_run {
        println!("Dry run complete. No layers or catalog entries were written.");
    } else {
        println!("Build complete. {imported} image(s) imported.");
    }
    Ok(())
}

struct BuildContext<'a> {
    data_dir: &'a Path,
    offline: bool,
    dry_run: bool,
}

/// Imports one component image; returns whether an import happened.
fn build_component(
    context: &BuildContext<'_>,
    name: &str,
    reference: &ImageReference,
) -> anyhow::Result<bool> {
    println!("  {name} -> {reference}");
    if reference.scheme() == ImageScheme::Catalog {
        let catalog = containust_image::registry::ImageCatalog::open(context.data_dir)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let entry = catalog
            .find(reference.location())
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        println!(
            "    Already imported (digest {})",
            entry.digest.as_deref().unwrap_or("<none>")
        );
        return Ok(false);
    }
    if context.dry_run {
        if reference.scheme() == ImageScheme::Preset {
            let preset = resolve_preset(reference).map_err(|e| anyhow::anyhow!("{e}"))?;
            println!(
                "    Would download {} ({}) → sha256:{}",
                preset.url, preset.description, preset.sha256
            );
        } else {
            println!(
                "    Would import as '{name}' (cache key {})",
                reference.cache_key()
            );
        }
        return Ok(false);
    }
    let request = ImportRequest::new(name, context.offline);
    let entry =
        import_image(context.data_dir, reference, &request).map_err(|e| anyhow::anyhow!("{e}"))?;
    println!(
        "    Imported as image://{name}@sha256:{}",
        entry.digest.as_deref().unwrap_or_default()
    );
    Ok(true)
}
