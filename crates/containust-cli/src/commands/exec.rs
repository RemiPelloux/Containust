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
/// Joins the target container's namespaces and runs the specified
/// command, forwarding stdout/stderr.
///
/// # Errors
///
/// Returns an error if the container is not running or namespace joining fails.
pub fn execute(args: ExecArgs, options: &super::RuntimeOptions) -> anyhow::Result<()> {
    let engine = options.engine();
    let id = super::resolve_container_id(&engine, &args.container)?;
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
