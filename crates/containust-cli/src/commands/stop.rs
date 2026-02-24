//! `ctst stop` — Stop containers and clean up resources.

use clap::Args;
use containust_common::types::ContainerId;
use containust_runtime::engine::Engine;

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
/// Stops individual containers by ID/name, or all containers
/// if none are specified.
///
/// # Errors
///
/// Returns an error if container stopping or cleanup fails.
pub fn execute(args: StopArgs) -> anyhow::Result<()> {
    let engine = Engine::new();

    if args.containers.is_empty() {
        engine.stop_all().map_err(|e| anyhow::anyhow!("{e}"))?;
        println!("All containers stopped.");
    } else {
        for name in &args.containers {
            let id = ContainerId::new(name);
            engine.stop(&id).map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("Stopped: {name}");
        }
    }

    Ok(())
}
