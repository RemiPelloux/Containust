//! `ctst exec` — Execute a command inside a running container.

use clap::Args;
use containust_common::types::ContainerId;
use containust_runtime::engine::Engine;

/// Arguments for the `exec` command.
#[derive(Args, Debug)]
pub struct ExecArgs {
    /// Container ID or name.
    pub container: String,

    /// Command to execute.
    #[arg(trailing_var_arg = true, required = true)]
    pub command: Vec<String>,
}

/// Executes the `exec` command.
///
/// Joins the target container's namespaces and runs the specified
/// command, forwarding stdout/stderr.
///
/// # Errors
///
/// Returns an error if the container is not running or namespace joining fails.
pub fn execute(args: ExecArgs) -> anyhow::Result<()> {
    let engine = Engine::new();
    let id = resolve_target(&engine, &args.container)?;
    let output = engine
        .exec(&id, &args.command)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if !output.stdout.is_empty() {
        print!("{}", output.stdout);
    }
    if !output.stderr.is_empty() {
        #[allow(clippy::print_stderr)]
        {
            eprint!("{}", output.stderr);
        }
    }

    std::process::exit(output.exit_code);
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
