//! Full container lifecycle management via the Containust SDK.
//!
//! Demonstrates creating, configuring, inspecting, and managing a container
//! entirely through the Rust API.
//!
//! Run with:
//! ```bash
//! cargo run --example sdk_lifecycle
//! ```

use containust_common::error::ContainustError;
use containust_sdk::builder::ContainerBuilder;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    tracing::info!("=== Containust SDK: Container Lifecycle ===");

    let container = ContainerBuilder::new("lifecycle-demo")
        .image("file:///opt/images/alpine")
        .command(vec![
            "/bin/sh".into(),
            "-c".into(),
            "echo 'Starting...'; sleep 5; echo 'Done.'".into(),
        ])
        .env("APP_NAME", "lifecycle-demo")
        .env("LOG_LEVEL", "debug")
        .memory_limit(128 * 1024 * 1024)
        .cpu_shares(1024)
        .readonly_rootfs(true)
        .build()?;

    tracing::info!(id = %container.id, state = %container.state, "Container created");
    tracing::info!(command = ?container.command, "Command");
    tracing::info!(
        memory = ?container.limits.memory_bytes,
        cpu = ?container.limits.cpu_shares,
        "Resource limits"
    );

    for (key, value) in &container.env {
        tracing::info!(key, value, "Environment variable");
    }

    match ContainerBuilder::new("missing-image").build() {
        Ok(_) => tracing::warn!("Expected an error for missing image"),
        Err(ContainustError::Config { message }) => {
            tracing::info!(message, "Correctly caught missing image error");
        }
        Err(e) => tracing::error!(%e, "Unexpected error type"),
    }

    tracing::info!("=== Lifecycle demo complete ===");
    Ok(())
}
