//! `ctst run` — Deploy and run the component graph.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use clap::Args;
use containust_runtime::engine::{ApplyOptions, DeployedComponent, Engine};

/// Arguments for the `run` command.
#[derive(Args, Debug)]
pub struct RunArgs {
    /// Path to the .ctst composition file.
    #[arg(default_value = "containust.ctst")]
    pub file: String,

    /// Run in detached mode (don't wait for Ctrl+C).
    #[arg(short, long)]
    pub detach: bool,

    /// Stop and remove containers that are no longer declared in the file.
    #[arg(long)]
    pub prune: bool,
}

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const GREEN: &str = "\x1b[32m";
const CYAN: &str = "\x1b[36m";
const YELLOW: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";

/// Executes the `run` command.
///
/// # Errors
///
/// Returns an error if deployment fails.
pub fn execute(args: RunArgs, options: &super::RuntimeOptions) -> anyhow::Result<()> {
    let total_start = Instant::now();
    print_header();

    let path = std::path::Path::new(&args.file);
    if !path.exists() {
        return Err(anyhow::anyhow!(
            "Composition file not found: {}\n\
             Create a .ctst file or specify a path: ctst run <file>",
            args.file
        ));
    }

    let engine = options.engine_for_project(path);
    if !engine.is_available() {
        print_vm_notice();
    }

    let deployed = deploy_and_report(&engine, path, args.prune, total_start)?;

    if args.detach {
        eprintln!();
        eprintln!("  Running detached. Use {BOLD}ctst stop{RESET} to stop all containers.");
        return Ok(());
    }

    wait_for_shutdown(&engine, &deployed)
}

fn print_header() {
    eprintln!();
    eprintln!(
        "  {BOLD}Containust{RESET} {DIM}v{}{RESET}",
        env!("CARGO_PKG_VERSION")
    );
    eprintln!();
}

fn print_vm_notice() {
    eprintln!("  {YELLOW}Note:{RESET} No native container support on this OS.");
    eprintln!("        A lightweight Linux VM will be used (requires QEMU).");
    eprintln!();
}

fn deploy_and_report(
    engine: &Engine,
    path: &Path,
    prune: bool,
    total_start: Instant,
) -> anyhow::Result<Vec<DeployedComponent>> {
    let outcome = engine
        .apply(path, ApplyOptions { prune })
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let deployed = outcome.deployed;
    for warning in &outcome.plan.warnings {
        eprintln!("  {YELLOW}warning:{RESET} {warning}");
    }

    eprintln!();
    eprintln!(
        "  {GREEN}{BOLD}Applied{RESET} {} in {:.1}s:",
        outcome.plan.summary(),
        total_start.elapsed().as_secs_f64()
    );
    eprintln!();

    for comp in &deployed {
        let port_info = comp.port.map_or_else(String::new, |p| {
            format!(" {CYAN}->{RESET} http://localhost:{p}")
        });
        eprintln!(
            "    {GREEN}●{RESET} {BOLD}{}{RESET} {DIM}[{}]{RESET}{port_info}",
            comp.name, comp.id
        );
    }

    let ports: Vec<_> = deployed.iter().filter_map(|c| c.port).collect();
    if !ports.is_empty() {
        eprintln!();
        for port in &ports {
            eprintln!("  {CYAN}Access at:{RESET} {BOLD}http://localhost:{port}{RESET}");
        }
    }

    for name in &outcome.removed {
        eprintln!("    {DIM}-{RESET} {name} {DIM}(removed){RESET}");
    }
    let undeclared = outcome
        .plan
        .count(containust_runtime::plan::PlanAction::Remove)
        - outcome.removed.len();
    if undeclared > 0 {
        eprintln!();
        eprintln!(
            "  {YELLOW}Note:{RESET} {undeclared} undeclared container(s) left in place; \
             rerun with {BOLD}--prune{RESET} to remove them."
        );
    }

    let project_dir = containust_common::constants::project_dir(path);
    eprintln!();
    eprintln!("  {DIM}Project state: {}{RESET}", project_dir.display());

    Ok(deployed)
}

fn wait_for_shutdown(engine: &Engine, _deployed: &[DeployedComponent]) -> anyhow::Result<()> {
    eprintln!();
    eprintln!("  Press {BOLD}Ctrl+C{RESET} to stop all containers...");

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .map_err(|e| anyhow::anyhow!("failed to set Ctrl+C handler: {e}"))?;

    while running.load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(250));
    }

    eprintln!();
    eprintln!("  Stopping containers...");
    engine.stop_all().map_err(|e| anyhow::anyhow!("{e}"))?;
    eprintln!("  {GREEN}All containers stopped.{RESET}");

    Ok(())
}
