//! `ctst ps` — List running containers with real-time metrics.

use clap::Args;
use containust_runtime::engine::Engine;

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
/// Queries the engine for running containers and displays them
/// in a tabular format.
///
/// # Errors
///
/// Returns an error if state loading or TUI initialization fails.
pub fn execute(args: PsArgs) -> anyhow::Result<()> {
    let engine = Engine::new();
    let containers = engine.list().map_err(|e| anyhow::anyhow!("{e}"))?;

    let filtered: Vec<_> = if args.all {
        containers
    } else {
        containers
            .into_iter()
            .filter(|c| c.state == "running")
            .collect()
    };

    if filtered.is_empty() {
        println!("No containers found.");
        return Ok(());
    }

    println!(
        "{:<40} {:<15} {:<10} {:<8} {:<20}",
        "CONTAINER ID", "NAME", "STATE", "PID", "IMAGE"
    );
    for c in &filtered {
        println!(
            "{:<40} {:<15} {:<10} {:<8} {:<20}",
            c.id,
            c.name,
            c.state,
            c.pid.map_or_else(|| "-".to_string(), |p| p.to_string()),
            c.image
        );
    }

    Ok(())
}
