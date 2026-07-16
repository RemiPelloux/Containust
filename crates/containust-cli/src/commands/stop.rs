//! `ctst stop` — Stop containers and clean up resources.

use clap::Args;
use containust_common::types::ContainerId;
use containust_runtime::engine::Engine;

/// Arguments for the `stop` command.
#[derive(Args, Debug)]
pub struct StopArgs {
    /// Container IDs or names to stop. If empty, stops all.
    pub containers: Vec<String>,

    /// Force kill without graceful shutdown.
    #[arg(short, long)]
    pub force: bool,
}

/// Executes the `stop` command.
///
/// Stops individual containers by ID/name, or all containers
/// if none are specified.
///
/// # Errors
///
/// Returns an error if container stopping or cleanup fails.
pub fn execute(args: StopArgs) -> anyhow::Result<()> {
    let engine = Engine::new();

    if args.containers.is_empty() {
        engine.stop_all().map_err(|e| anyhow::anyhow!("{e}"))?;
        println!("All containers stopped.");
    } else {
        let containers = engine.list().map_err(|e| anyhow::anyhow!("{e}"))?;
        for name in &args.containers {
            let id = containers
                .iter()
                .find(|container| container.id.as_str() == name || container.name == *name)
                .map_or_else(|| ContainerId::new(name), |container| container.id.clone());
            engine.stop(&id).map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("Stopped: {name}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use containust_runtime::backend::ContainerInfo;

    #[test]
    fn resolve_stop_target_prefers_name_or_id_match() {
        let id = ContainerId::new("id-1");
        let containers = [ContainerInfo {
            id: id.clone(),
            name: "web".into(),
            state: "running".into(),
            pid: Some(1),
            image: "file:///image".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
        }];

        assert_eq!(resolve_target("web", &containers), id);
        assert_eq!(resolve_target("id-1", &containers), id);
        assert_eq!(
            resolve_target("missing", &containers),
            ContainerId::new("missing")
        );
    }

    fn resolve_target(target: &str, containers: &[ContainerInfo]) -> ContainerId {
        containers
            .iter()
            .find(|container| container.id.as_str() == target || container.name == target)
            .map_or_else(
                || ContainerId::new(target),
                |container| container.id.clone(),
            )
    }
}
