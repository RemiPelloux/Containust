//! `ctst convert` — Convert a docker-compose.yml to `.ctst` format.

use std::path::PathBuf;

use clap::Args;

/// Arguments for the `convert` subcommand.
#[derive(Args, Debug)]
pub struct ConvertArgs {
    /// Path to the docker-compose.yml file.
    #[arg(default_value = "docker-compose.yml")]
    pub file: PathBuf,

    /// Write output to a file instead of stdout.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

/// Executes the `convert` command.
///
/// # Errors
///
/// Returns an error if the file cannot be read, parsed, or written.
pub fn execute(args: ConvertArgs) -> anyhow::Result<()> {
    let input = &args.file;
    tracing::info!(path = %input.display(), "converting docker-compose file");

    if !input.exists() {
        anyhow::bail!("file not found: {}", input.display());
    }

    let ctst_output = crate::converter::convert_file(input)?;

    if let Some(ref out_path) = args.output {
        std::fs::write(out_path, &ctst_output)?;
        println!("Converted {} -> {}", input.display(), out_path.display());
        println!("Components: {}", ctst_output.matches("COMPONENT ").count());
        println!("Connections: {}", ctst_output.matches("CONNECT ").count());
    } else {
        print!("{ctst_output}");
    }

    Ok(())
}
