//! `ctst plan` — Display planned infrastructure changes before applying.

use clap::Args;

/// Arguments for the `plan` command.
#[derive(Args, Debug)]
pub struct PlanArgs {
    /// Path to the .ctst composition file.
    #[arg(default_value = "containust.ctst")]
    pub file: String,
}

/// Executes the `plan` command.
///
/// Parses the `.ctst` file, builds the dependency graph, resolves
/// topological order, and displays a deployment plan.
///
/// # Errors
///
/// Returns an error if parsing, validation, or graph resolution fails.
pub fn execute(args: PlanArgs) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(&args.file)?;
    let composition =
        containust_compose::parser::parse_ctst(&content).map_err(|e| anyhow::anyhow!("{e}"))?;

    let mut graph = containust_compose::graph::DependencyGraph::new();
    let mut node_map = std::collections::HashMap::new();

    for comp in &composition.components {
        let idx = graph.add_component(&comp.name);
        let _ = node_map.insert(comp.name.clone(), idx);
    }
    for conn in &composition.connections {
        if let (Some(&from), Some(&to)) = (node_map.get(&conn.from), node_map.get(&conn.to)) {
            graph.add_dependency(from, to);
        }
    }

    let order = graph.resolve_order().map_err(|e| anyhow::anyhow!("{e}"))?;

    println!("Deployment Plan for: {}", args.file);
    println!(
        "\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}"
    );
    println!();

    for name in &order {
        let comp = composition.components.iter().find(|c| c.name == *name);
        if let Some(c) = comp {
            println!("  + {name}");
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
    }

    println!();
    println!("  {} component(s) will be deployed.", order.len());

    if !composition.connections.is_empty() {
        println!();
        println!("  Connections:");
        for conn in &composition.connections {
            println!("    {} -> {}", conn.from, conn.to);
        }
    }

    Ok(())
}
