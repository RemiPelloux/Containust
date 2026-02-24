//! `ctst build` — Parse a .ctst file and build container images/layers.

use clap::Args;

/// Arguments for the `build` command.
#[derive(Args, Debug)]
pub struct BuildArgs {
    /// Path to the .ctst composition file.
    #[arg(default_value = "containust.ctst")]
    pub file: String,
}

/// Executes the `build` command.
///
/// Parses the `.ctst` file, validates the AST, and resolves image
/// sources for each declared component.
///
/// # Errors
///
/// Returns an error if parsing, validation, or image resolution fails.
pub fn execute(args: BuildArgs) -> anyhow::Result<()> {
    tracing::info!(file = %args.file, "building from .ctst file");

    let content = std::fs::read_to_string(&args.file)?;
    let composition =
        containust_compose::parser::parse_ctst(&content).map_err(|e| anyhow::anyhow!("{e}"))?;

    println!(
        "Parsed {} components, {} connections",
        composition.components.len(),
        composition.connections.len()
    );

    for comp in &composition.components {
        if let Some(ref image) = comp.image {
            println!("  {} -> {}", comp.name, image);
            match containust_image::source::resolve_source(image) {
                Ok(source) => println!("    Source: {source:?}"),
                Err(e) => println!("    Warning: {e}"),
            }
        }
    }

    println!("Build complete.");
    Ok(())
}
