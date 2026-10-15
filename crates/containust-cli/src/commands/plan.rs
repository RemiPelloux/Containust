//! `ctst plan` — Display planned infrastructure changes before applying.

use clap::Args;
use containust_runtime::plan::PlanAction;

/// Arguments for the `plan` command.
#[derive(Args, Debug)]
pub struct PlanArgs {
    /// Path to the .ctst composition file.
    #[arg(default_value = "containust.ctst")]
    pub file: String,

    /// Print the plan as JSON instead of a human-readable diff.
    #[arg(long)]
    pub json: bool,

    /// Exit with status 2 when the plan contains changes (CI drift check).
    #[arg(long)]
    pub detailed_exitcode: bool,
}

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

/// Executes the `plan` command.
///
/// Parses the `.ctst` file, resolves dependency order, and diffs the
/// desired components against live containers and the applied-spec ledger.
/// Nothing on the host is changed.
///
/// # Errors
///
/// Returns an error if parsing, validation, graph resolution, or state
/// inspection fails.
pub fn execute(args: PlanArgs, options: &super::RuntimeOptions) -> anyhow::Result<()> {
    let path = std::path::Path::new(&args.file);
    let content = std::fs::read_to_string(path)?;
    let composition =
        containust_compose::parser::parse_ctst(&content).map_err(|e| anyhow::anyhow!("{e}"))?;
    let engine = options.engine_for_project(path);
    let plan = engine.plan(path).map_err(|e| anyhow::anyhow!("{e}"))?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        print_plan(&args.file, &composition, &plan);
    }

    if args.detailed_exitcode && !plan.is_noop() {
        std::process::exit(2);
    }
    Ok(())
}

fn print_component_details(
    composition: &containust_compose::parser::ast::CompositionFile,
    name: &str,
) {
    let Some(c) = composition.components.iter().find(|c| c.name == name) else {
        return;
    };
    if let Some(ref img) = c.image {
        println!("      image: {img}");
    }
    if let Some(p) = c.port {
        println!("      port: {p}");
    }
    if let Some(ref mem) = c.memory {
        println!("      memory: {mem}");
    }
}

fn print_plan(
    file: &str,
    composition: &containust_compose::parser::ast::CompositionFile,
    plan: &containust_runtime::plan::DeployPlan,
) {
    println!("{BOLD}Plan for {file}{RESET}");
    println!("{DIM}{}{RESET}", "\u{2500}".repeat(48));

    for change in &plan.changes {
        let color = match change.action {
            PlanAction::Create => GREEN,
            PlanAction::Replace => YELLOW,
            PlanAction::Start => CYAN,
            PlanAction::Keep => DIM,
            PlanAction::Remove => RED,
        };
        let reason = change
            .reason
            .as_deref()
            .map_or_else(String::new, |r| format!(" {DIM}({r}){RESET}"));
        println!(
            "  {color}{} {:<8}{RESET} {BOLD}{}{RESET}{reason}",
            change.action.symbol(),
            change.action.to_string(),
            change.name
        );
        if matches!(change.action, PlanAction::Create | PlanAction::Replace) {
            print_component_details(composition, &change.name);
        }
    }

    if !composition.connections.is_empty() {
        println!();
        println!("  Connections:");
        for conn in &composition.connections {
            println!("    {} -> {}", conn.from, conn.to);
        }
    }

    println!();
    if plan.is_noop() {
        println!("  {GREEN}No changes.{RESET} Running stack matches {file}.");
    } else {
        println!("  {BOLD}Plan:{RESET} {}.", plan.summary());
        if plan.count(PlanAction::Remove) > 0 {
            println!("  {DIM}Removals apply only with `ctst run --prune`.{RESET}");
        }
    }
}
