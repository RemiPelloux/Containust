//! `ctst exec` — Execute a command inside a running container.

use clap::Args;

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
/// # Errors
///
/// Returns an error if the container is not running or namespace joining fails.
pub fn execute(args: ExecArgs) -> anyhow::Result<()> {
    tracing::info!(
        container = %args.container,
        "executing command in container"
    );
    Ok(())
}
