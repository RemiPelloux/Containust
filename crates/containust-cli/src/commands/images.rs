//! `ctst images` — Manage the local image catalog.

use clap::Args;

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
/// # Errors
///
/// Returns an error if catalog operations fail.
pub fn execute(_args: ImagesArgs) -> anyhow::Result<()> {
    tracing::info!("managing images");
    println!("IMAGE ID\tNAME\tLAYERS\tSIZE");
    Ok(())
}
