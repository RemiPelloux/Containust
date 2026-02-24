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
