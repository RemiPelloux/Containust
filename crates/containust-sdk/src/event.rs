//! Container lifecycle event streaming via the runtime event bus.

use std::sync::mpsc;

use containust_common::types::{ContainerId, ContainerState};
pub use containust_runtime::events::{EventBus, LifecycleEvent};

/// A container lifecycle event (SDK-facing aliases for runtime events).
#[derive(Debug, Clone)]
pub enum ContainerEvent {
    /// A container changed state.
    StateChange {
        /// Container that changed.
        container_id: ContainerId,
        /// Previous state.
        from: ContainerState,
        /// New state.
        to: ContainerState,
    },
    /// A metrics update for a container.
    MetricsUpdate {
        /// Container this update belongs to.
        container_id: ContainerId,
    },
    /// A timed operation completed.
    Operation {
        /// Optional target container.
        container_id: Option<ContainerId>,
        /// Project identity.
        project: String,
        /// Operation name.
        operation: String,
        /// Duration in milliseconds.
        duration_ms: u64,
        /// Error catalog code on failure.
        error_code: Option<String>,
    },
}

impl From<LifecycleEvent> for ContainerEvent {
    fn from(event: LifecycleEvent) -> Self {
        match event {
            LifecycleEvent::Operation {
                container_id,
                project,
                operation,
                duration_ms,
                error_code,
            } => Self::Operation {
                container_id: container_id.map(ContainerId::new),
                project,
                operation,
                duration_ms,
                error_code,
            },
            LifecycleEvent::StateChange {
                container_id,
                from,
                to,
            } => Self::StateChange {
                container_id: ContainerId::new(container_id),
                from: parse_state(&from),
                to: parse_state(&to),
            },
        }
    }
}

fn parse_state(label: &str) -> ContainerState {
    match label.to_ascii_lowercase().as_str() {
        "running" => ContainerState::Running,
        "stopped" => ContainerState::Stopped,
        "failed" => ContainerState::Failed,
        _ => ContainerState::Created,
    }
}

/// Listens for container lifecycle events from an [`EventBus`].
#[derive(Debug)]
pub struct EventListener {
    rx: Option<mpsc::Receiver<LifecycleEvent>>,
}

impl EventListener {
    /// Creates a listener that is not yet subscribed.
    #[must_use]
    pub const fn new() -> Self {
        Self { rx: None }
    }

    /// Subscribes to `bus` and returns a listener that can poll events.
    #[must_use]
    pub fn subscribe(bus: &EventBus) -> Self {
        Self {
            rx: Some(bus.subscribe()),
        }
    }

    /// Tries to receive the next event without blocking.
    pub fn try_recv(&self) -> Option<ContainerEvent> {
        self.rx.as_ref()?.try_recv().ok().map(ContainerEvent::from)
    }
}

impl Default for EventListener {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(
    clippy::panic,
    clippy::expect_used,
    clippy::match_wildcard_for_single_variants
)]
mod tests {
    use super::*;

    #[test]
    fn event_listener_subscribes_to_bus() {
        let bus = EventBus::new();
        let listener = EventListener::subscribe(&bus);
        bus.emit_operation(containust_runtime::events::OperationEmit {
            project: "demo".into(),
            operation: "deploy".into(),
            duration_ms: 5,
            container_id: None,
            error_code: Some("R001"),
        });
        let event = listener.try_recv().expect("event");
        match event {
            ContainerEvent::Operation {
                project,
                operation,
                error_code,
                ..
            } => {
                assert_eq!(project, "demo");
                assert_eq!(operation, "deploy");
                assert_eq!(error_code.as_deref(), Some("R001"));
            }
            _ => panic!("expected operation"),
        }
    }

    #[test]
    fn container_event_state_change_variants() {
        let id = ContainerId::new("test-1");
        let event = ContainerEvent::StateChange {
            container_id: id.clone(),
            from: ContainerState::Created,
            to: ContainerState::Running,
        };
        match event {
            ContainerEvent::StateChange {
                container_id,
                from,
                to,
            } => {
                assert_eq!(container_id.as_str(), id.as_str());
                assert_eq!(from, ContainerState::Created);
                assert_eq!(to, ContainerState::Running);
            }
            _ => panic!("expected StateChange"),
        }
    }
}
