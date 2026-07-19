//! `ctst stop` — Stop containers and clean up resources.

use clap::Args;

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
pub fn execute(args: StopArgs, options: &super::RuntimeOptions) -> anyhow::Result<()> {
    let engine = options.engine();

    if args.containers.is_empty() {
        engine
            .stop_all_with_force(args.force)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        println!("All containers stopped.");
    } else {
        let containers = engine.list().map_err(|e| anyhow::anyhow!("{e}"))?;
        for name in &args.containers {
            let id = super::resolve_container_id_from(&containers, name)?;
            engine
                .stop_with_force(&id, args.force)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("Stopped: {name}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use containust_common::types::ContainerId;
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

        assert_eq!(
            super::super::resolve_container_id_from(&containers, "web").expect("web"),
            id
        );
        assert_eq!(
            super::super::resolve_container_id_from(&containers, "id-1").expect("id"),
            id
        );
        assert!(super::super::resolve_container_id_from(&containers, "missing").is_err());
    }
}
