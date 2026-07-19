//! `ctst rm` — Remove containers and project-owned resources.

use clap::Args;

/// Arguments for the `rm` command.
#[derive(Args, Debug)]
pub struct RemoveArgs {
    /// Container IDs or names to remove.
    #[arg(required = true)]
    pub containers: Vec<String>,

    /// Stop running containers before removing them.
    #[arg(short, long)]
    pub force: bool,
}

/// Executes the `rm` command.
///
/// # Errors
///
/// Returns an error when a target is missing, running without `--force`, or
/// one of its project-owned resources cannot be removed.
pub fn execute(args: RemoveArgs, options: &super::RuntimeOptions) -> anyhow::Result<()> {
    let engine = options.engine();
    let containers = engine.list().map_err(|error| anyhow::anyhow!("{error}"))?;

    for target in &args.containers {
        let id = super::resolve_container_id_from(&containers, target)?;
        let running = containers
            .iter()
            .find(|container| container.id == id)
            .is_some_and(|container| container.state == "running");
        if running {
            if !args.force {
                return Err(anyhow::anyhow!(
                    "container {target} is running; stop it first or use --force"
                ));
            }
            engine
                .stop_with_force(&id, true)
                .map_err(|error| anyhow::anyhow!("{error}"))?;
        }
        engine
            .remove(&id)
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        println!("Removed: {target}");
    }

    Ok(())
}
