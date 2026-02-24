//! `ctst ps` — List running containers with real-time metrics.

use clap::Args;

use crate::output;

/// Arguments for the `ps` command.
#[derive(Args, Debug)]
pub struct PsArgs {
    /// Show all containers (including stopped).
    #[arg(short, long)]
    pub all: bool,

    /// Launch the interactive TUI dashboard.
    #[arg(long)]
    pub tui: bool,
}

/// Executes the `ps` command.
///
/// # Errors
///
/// Returns an error if state loading or TUI initialization fails.
pub fn execute(_args: PsArgs) -> anyhow::Result<()> {
    tracing::info!("listing containers");
    println!(
        "CONTAINER ID\tSTATUS\tCPU\tMEMORY\n(no containers, max memory display: {})",
        output::format_bytes(0)
    );
    Ok(())
}
