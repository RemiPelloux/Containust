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
/// # Errors
///
/// Returns an error if parsing or image building fails.
pub fn execute(_args: BuildArgs) -> anyhow::Result<()> {
    tracing::info!(file = %_args.file, "building from .ctst file");
    println!("Building from: {}", _args.file);
    Ok(())
}
