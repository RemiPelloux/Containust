//! `ctst run` — Deploy the component graph.

use clap::Args;
use containust_runtime::engine::Engine;

/// Arguments for the `run` command.
#[derive(Args, Debug)]
pub struct RunArgs {
    /// Path to the .ctst composition file.
    #[arg(default_value = "containust.ctst")]
    pub file: String,

    /// Run in detached mode.
    #[arg(short, long)]
    pub detach: bool,
}

/// Executes the `run` command.
///
/// Creates an engine instance, checks backend availability,
/// and deploys all components from the `.ctst` file.
///
/// # Errors
///
/// Returns an error if deployment fails.
pub fn execute(args: RunArgs) -> anyhow::Result<()> {
    let engine = Engine::new();

    if !engine.is_available() {
        println!("Warning: Native container backend not available on this platform.");
        println!("A lightweight Linux VM will be used (requires QEMU).");
    }

    let path = std::path::Path::new(&args.file);
    let ids = engine.deploy(path).map_err(|e| anyhow::anyhow!("{e}"))?;

    println!("Deployed {} container(s):", ids.len());
    for id in &ids {
        println!("  {id}");
    }

    Ok(())
}
