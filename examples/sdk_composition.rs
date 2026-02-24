//! Composition deployment via the Containust SDK.
//!
//! Demonstrates loading a `.ctst` composition file, resolving the
//! dependency graph, and creating containers in deployment order.
//!
//! Run with:
//! ```bash
//! cargo run --example sdk_composition
//! ```

use std::path::Path;

use containust_common::error::ContainustError;
use containust_sdk::builder::ContainerBuilder;
use containust_sdk::graph_resolver::GraphResolver;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    tracing::info!("=== Containust SDK: Composition Deployment ===");

    let mut resolver = GraphResolver::new();

    let ctst_path = Path::new("examples/web_stack.ctst");
    tracing::info!(path = %ctst_path.display(), "Loading composition");

    if let Err(e) = resolver.load_ctst(ctst_path) {
        tracing::error!(%e, "Failed to load composition file");
        return Err(e.into());
    }

    let order = resolver.deployment_order()?;
    tracing::info!(count = order.len(), "Resolved deployment order");

    for (idx, component) in order.iter().enumerate() {
        tracing::info!(step = idx + 1, component, "Deploying component");

        let container = ContainerBuilder::new(component.as_str())
            .image(format!("file:///opt/images/{component}"))
            .memory_limit(64 * 1024 * 1024)
            .cpu_shares(512)
            .readonly_rootfs(true)
            .build()?;

        tracing::info!(
            id = %container.id,
            state = %container.state,
            "Container ready"
        );
    }

    match GraphResolver::new().load_ctst(Path::new("nonexistent.ctst")) {
        Ok(()) => tracing::warn!("Expected error for missing file"),
        Err(ContainustError::Io { path, .. }) => {
            tracing::info!(?path, "Correctly caught missing file error");
        }
        Err(e) => tracing::error!(%e, "Unexpected error type"),
    }

    tracing::info!("=== Composition deployment complete ===");
    Ok(())
}
