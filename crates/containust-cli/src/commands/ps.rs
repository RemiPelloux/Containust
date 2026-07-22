//! `ctst ps` — List running containers with real-time metrics.

use clap::Args;
use containust_runtime::metrics::{MetricAvailability, collect_metrics};

/// Arguments for the `ps` command.
#[derive(Args, Debug)]
pub struct PsArgs {
    /// Show all containers (including stopped).
    #[arg(short, long)]
    pub all: bool,

    /// Launch the interactive TUI dashboard.
    #[arg(long)]
    pub tui: bool,
}

/// Executes the `ps` command.
///
/// # Errors
///
/// Returns an error if state loading or TUI initialization fails.
pub fn execute(args: PsArgs, options: &super::RuntimeOptions) -> anyhow::Result<()> {
    let engine = options.engine();
    let (containers, reconciliation) = engine
        .list_reconciled()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    print_reconciliation(&reconciliation);

    let filtered: Vec<_> = if args.all {
        containers
    } else {
        containers
            .into_iter()
            .filter(|c| c.state == "running")
            .collect()
    };

    if args.tui {
        let rows: Vec<containust_tui::ContainerRow> = filtered
            .into_iter()
            .map(|c| containust_tui::ContainerRow {
                id: c.id.to_string(),
                name: c.name,
                state: c.state,
                pid: c.pid.map_or_else(|| "-".into(), |p| p.to_string()),
                image: c.image,
            })
            .collect();
        return containust_tui::run_dashboard(&rows).map_err(Into::into);
    }

    if filtered.is_empty() {
        println!("No containers found.");
        return Ok(());
    }

    println!(
        "{:<36} {:<14} {:<10} {:<8} {:>10} {:>10} {:<20}",
        "CONTAINER ID", "NAME", "STATE", "PID", "CPU(ns)", "MEM(B)", "IMAGE"
    );
    for c in &filtered {
        let (cpu, mem) = format_metrics(&c.id);
        println!(
            "{:<36} {:<14} {:<10} {:<8} {:>10} {:>10} {:<20}",
            c.id,
            c.name,
            c.state,
            c.pid.map_or_else(|| "-".to_string(), |p| p.to_string()),
            cpu,
            mem,
            c.image
        );
    }

    Ok(())
}

fn print_reconciliation(reconciliation: &containust_runtime::backend::ReconciliationReport) {
    if reconciliation.stale_processes > 0
        || reconciliation.orphaned_rootfs > 0
        || reconciliation.orphaned_cgroups > 0
    {
        eprintln!(
            "Reconciled: {} stale process(es), {} orphaned rootfs, {} orphaned cgroup(s)",
            reconciliation.stale_processes,
            reconciliation.orphaned_rootfs,
            reconciliation.orphaned_cgroups
        );
    }
}

fn format_metrics(id: &containust_common::types::ContainerId) -> (String, String) {
    match collect_metrics(id) {
        Ok(snap) => {
            let cpu = match snap.cpu {
                MetricAvailability::Available => snap.cpu_usage_ns.to_string(),
                MetricAvailability::Unavailable | MetricAvailability::Missing => "-".into(),
            };
            let mem = match snap.memory {
                MetricAvailability::Available => snap.memory_usage_bytes.to_string(),
                MetricAvailability::Unavailable | MetricAvailability::Missing => "-".into(),
            };
            (cpu, mem)
        }
        Err(_) => ("-".into(), "-".into()),
    }
}
