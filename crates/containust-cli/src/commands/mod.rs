//! CLI command definitions and dispatch.

pub mod build;
pub mod convert;
pub mod exec;
pub mod images;
pub mod logs;
pub mod plan;
pub mod ps;
pub mod run;
pub mod stop;

use clap::{Parser, Subcommand};

/// Containust — Daemon-less sovereign container runtime.
#[derive(Parser, Debug)]
#[command(name = "ctst", version, about, long_about = None)]
pub struct Cli {
    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Command,

    /// Enable offline mode (block all network access).
    #[arg(long, global = true)]
    pub offline: bool,

    /// Path to the state file.
    #[arg(long, global = true, default_value = containust_common::constants::DEFAULT_STATE_FILE)]
    pub state_file: String,
}

/// Available CLI subcommands.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Parse a .ctst file and build container images/layers.
    Build(build::BuildArgs),
    /// Display the planned infrastructure changes before applying.
    Plan(plan::PlanArgs),
    /// Deploy the component graph.
    Run(run::RunArgs),
    /// List running containers with real-time metrics.
    Ps(ps::PsArgs),
    /// Execute a command inside a running container.
    Exec(exec::ExecArgs),
    /// Stop containers and clean up resources.
    Stop(stop::StopArgs),
    /// Manage the local image catalog.
    Images(images::ImagesArgs),
    /// Convert a docker-compose.yml to .ctst format.
    Convert(convert::ConvertArgs),
    /// View container logs.
    Logs(logs::LogsArgs),
}

/// Dispatches the parsed CLI command to its handler.
///
/// # Errors
///
/// Returns an error if the command execution fails.
pub fn execute(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Build(args) => build::execute(args),
        Command::Plan(args) => plan::execute(args),
        Command::Run(args) => run::execute(args),
        Command::Ps(args) => ps::execute(args),
        Command::Exec(args) => exec::execute(args),
        Command::Stop(args) => stop::execute(args),
        Command::Images(args) => images::execute(args),
        Command::Convert(args) => convert::execute(args),
        Command::Logs(args) => logs::execute(args),
    }
}
