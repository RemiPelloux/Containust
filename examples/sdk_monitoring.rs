//! Event monitoring via the Containust SDK.
//!
//! Demonstrates creating an event listener, processing container lifecycle
//! events, and filtering by container ID using structured logging.
//!
//! Run with:
//! ```bash
//! cargo run --example sdk_monitoring
//! ```

use containust_common::types::{ContainerId, ContainerState};
use containust_sdk::builder::ContainerBuilder;
use containust_sdk::event::{ContainerEvent, EventListener};

fn handle_event(event: &ContainerEvent) {
    match event {
        ContainerEvent::StateChange {
            container_id,
            from,
            to,
        } => {
            tracing::info!(
                container = %container_id,
                from = %from,
                to = %to,
                "State transition"
            );
        }
        ContainerEvent::MetricsUpdate { container_id } => {
            tracing::debug!(container = %container_id, "Metrics snapshot received");
        }
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("debug")
        .init();

    tracing::info!("=== Containust SDK: Event Monitoring ===");

    let _listener = EventListener::new();

    // Future API (not yet implemented):
    // listener.subscribe(|event| handle_event(&event));

    let containers: Vec<_> = ["web-server", "database", "cache"]
        .iter()
        .map(|name| {
            ContainerBuilder::new(*name)
                .image(format!("file:///opt/images/{name}"))
                .memory_limit(64 * 1024 * 1024)
                .cpu_shares(512)
                .readonly_rootfs(true)
                .build()
        })
        .collect::<Result<_, _>>()?;

    for c in &containers {
        tracing::info!(id = %c.id, state = %c.state, "Container registered");
    }

    let simulated_events = vec![
        ContainerEvent::StateChange {
            container_id: ContainerId::new("web-server"),
            from: ContainerState::Created,
            to: ContainerState::Running,
        },
        ContainerEvent::MetricsUpdate {
            container_id: ContainerId::new("database"),
        },
        ContainerEvent::StateChange {
            container_id: ContainerId::new("cache"),
            from: ContainerState::Created,
            to: ContainerState::Running,
        },
        ContainerEvent::StateChange {
            container_id: ContainerId::new("web-server"),
            from: ContainerState::Running,
            to: ContainerState::Stopped,
        },
    ];

    let watch_id = ContainerId::new("web-server");

    for event in &simulated_events {
        handle_event(event);

        let is_watched = match event {
            ContainerEvent::StateChange { container_id, .. }
            | ContainerEvent::MetricsUpdate { container_id } => *container_id == watch_id,
        };

        if is_watched {
            tracing::warn!(filter = %watch_id, "Matched watched container");
        }
    }

    tracing::info!("=== Monitoring demo complete ===");
    Ok(())
}
