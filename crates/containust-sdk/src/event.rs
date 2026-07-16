//! Container lifecycle event streaming.
//!
//! Provides an async event listener for monitoring container state
//! changes and metrics updates programmatically.

use containust_common::types::{ContainerId, ContainerState};

/// A container lifecycle event.
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
}

/// Listens for container lifecycle events.
#[derive(Debug)]
pub struct EventListener {
    _marker: std::marker::PhantomData<()>,
}

impl EventListener {
    /// Creates a new event listener.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl Default for EventListener {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::match_wildcard_for_single_variants)]
mod tests {
    use containust_common::types::{ContainerId, ContainerState};

    use super::*;

    #[test]
    fn event_listener_new_constructs() {
        let listener = EventListener::new();
        let debug = format!("{listener:?}");
        assert!(debug.contains("EventListener"));
    }

    #[test]
    fn event_listener_default_equals_new() {
        let a = EventListener::new();
        let b = EventListener::default();
        let debug_a = format!("{a:?}");
        let debug_b = format!("{b:?}");
        assert_eq!(debug_a, debug_b);
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
                assert_eq!(container_id, id);
                assert_eq!(from, ContainerState::Created);
                assert_eq!(to, ContainerState::Running);
            }
            ContainerEvent::MetricsUpdate { .. } => {
                panic!("expected StateChange variant");
            }
        }
    }

    #[test]
    fn container_event_metrics_update_variant() {
        let id = ContainerId::new("metrics-container");
        let event = ContainerEvent::MetricsUpdate {
            container_id: id.clone(),
        };

        match event {
            ContainerEvent::MetricsUpdate { container_id } => {
                assert_eq!(container_id, id);
            }
            ContainerEvent::StateChange { .. } => {
                panic!("expected MetricsUpdate variant");
            }
        }
    }

    #[test]
    fn container_event_clone_works() {
        let id = ContainerId::new("clone-test");
        let event = ContainerEvent::StateChange {
            container_id: id,
            from: ContainerState::Running,
            to: ContainerState::Stopped,
        };

        let cloned = event.clone();
        match (event, cloned) {
            (
                ContainerEvent::StateChange {
                    container_id: id1,
                    from: from1,
                    to: to1,
                },
                ContainerEvent::StateChange {
                    container_id: id2,
                    from: from2,
                    to: to2,
                },
            ) => {
                // ContainerId is Clone but not PartialEq in the same way; just check the fields
                assert_eq!(format!("{id1}"), format!("{id2}"));
                assert_eq!(from1, from2);
                assert_eq!(to1, to2);
            }
            _ => {
                panic!("expected StateChange variants");
            }
        }
    }

    #[test]
    fn container_event_all_state_transitions() {
        let transitions = [
            (ContainerState::Created, ContainerState::Running),
            (ContainerState::Running, ContainerState::Stopped),
            (ContainerState::Running, ContainerState::Failed),
            (ContainerState::Created, ContainerState::Failed),
        ];

        let id = ContainerId::new("transitions");
        for (from, to) in &transitions {
            let event = ContainerEvent::StateChange {
                container_id: id.clone(),
                from: *from,
                to: *to,
            };
            match event {
                ContainerEvent::StateChange { from: f, to: t, .. } => {
                    assert_eq!(f, *from);
                    assert_eq!(t, *to);
                }
                _ => panic!("unexpected variant"),
            }
        }
    }

    #[test]
    fn container_event_debug_output() {
        let id = ContainerId::new("debug-container");
        let event = ContainerEvent::MetricsUpdate { container_id: id };
        let debug = format!("{event:?}");
        assert!(debug.contains("MetricsUpdate"));
    }
}
