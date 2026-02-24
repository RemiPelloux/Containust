//! `ctst run` — Deploy the component graph.

use clap::Args;

/// Arguments for the `run` command.
#[derive(Args, Debug)]
pub struct RunArgs {
    /// Path to the .ctst composition file.
    #[arg(default_value = "containust.ctst")]
    pub file: String,

    /// Run in detached mode.
    #[arg(short, long)]
    pub detach: bool,
}

/// Executes the `run` command.
///
/// # Errors
///
/// Returns an error if deployment fails.
pub fn execute(args: RunArgs) -> anyhow::Result<()> {
    tracing::info!(file = %args.file, "deploying component graph");
    println!("Running: {}", args.file);
    Ok(())
}
