//! `ctst logs` — View container logs.

use clap::Args;
use containust_common::types::ContainerId;
use containust_runtime::engine::Engine;

/// Arguments for the `logs` command.
#[derive(Args, Debug)]
pub struct LogsArgs {
    /// Container ID or name.
    pub container: String,

    /// Follow log output.
    #[arg(short, long)]
    pub follow: bool,
}

/// Executes the `logs` command.
///
/// Retrieves and displays logs for the specified container.
///
/// # Errors
///
/// Returns an error if the container is not found or logs are unavailable.
pub fn execute(args: LogsArgs) -> anyhow::Result<()> {
    let engine = Engine::new();
    let id = resolve_target(&engine, &args.container)?;
    let logs = engine.logs(&id).map_err(|e| anyhow::anyhow!("{e}"))?;

    if logs.is_empty() {
        println!("No logs available for container: {}", args.container);
    } else {
        print!("{logs}");
    }

    Ok(())
}

fn resolve_target(engine: &Engine, target: &str) -> anyhow::Result<ContainerId> {
    let containers = engine.list().map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(containers
        .iter()
        .find(|container| container.id.as_str() == target || container.name == target)
        .map_or_else(
            || ContainerId::new(target),
            |container| container.id.clone(),
        ))
}
