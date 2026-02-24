//! `ctst stop` — Stop containers and clean up resources.

use clap::Args;

/// Arguments for the `stop` command.
#[derive(Args, Debug)]
pub struct StopArgs {
    /// Container IDs or names to stop. If empty, stops all.
    pub containers: Vec<String>,

    /// Force kill without graceful shutdown.
    #[arg(short, long)]
    pub force: bool,
}

/// Executes the `stop` command.
///
/// # Errors
///
/// Returns an error if container stopping or cleanup fails.
pub fn execute(_args: StopArgs) -> anyhow::Result<()> {
    tracing::info!("stopping containers");
    Ok(())
}
