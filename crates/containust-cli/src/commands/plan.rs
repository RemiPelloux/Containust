//! `ctst plan` — Display planned infrastructure changes before applying.

use clap::Args;

/// Arguments for the `plan` command.
#[derive(Args, Debug)]
pub struct PlanArgs {
    /// Path to the .ctst composition file.
    #[arg(default_value = "containust.ctst")]
    pub file: String,
}

/// Executes the `plan` command.
///
/// # Errors
///
/// Returns an error if parsing or plan computation fails.
pub fn execute(args: PlanArgs) -> anyhow::Result<()> {
    tracing::info!(file = %args.file, "computing plan");
    println!("Plan for: {}", args.file);
    Ok(())
}
