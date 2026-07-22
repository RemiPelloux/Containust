//! Structured lifecycle event bus for operator diagnostics.

use std::sync::{Mutex, mpsc};

use containust_common::types::ContainerId;
use serde::Serialize;

/// A structured runtime lifecycle event.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LifecycleEvent {
    /// A timed operation completed (success or failure).
    Operation {
        /// Optional container id when the operation targets one container.
        #[serde(skip_serializing_if = "Option::is_none")]
        container_id: Option<String>,
        /// Project identity (storage root basename or identifier).
        project: String,
        /// Operation name (`deploy`, `stop`, `exec`, …).
        operation: String,
        /// Wall-clock duration in milliseconds.
        duration_ms: u64,
        /// Stable error code when the operation failed.
        #[serde(skip_serializing_if = "Option::is_none")]
        error_code: Option<String>,
    },
    /// A container state transition.
    StateChange {
        /// Container that changed.
        container_id: String,
        /// Previous state label.
        from: String,
        /// New state label.
        to: String,
    },
}

/// Inputs for emitting a timed operation event.
#[derive(Debug, Clone)]
pub struct OperationEmit {
    /// Project identity.
    pub project: String,
    /// Operation name.
    pub operation: String,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Optional target container.
    pub container_id: Option<ContainerId>,
    /// Optional error catalog code.
    pub error_code: Option<&'static str>,
}

/// In-process broadcast bus for lifecycle events.
#[derive(Debug, Default)]
pub struct EventBus {
    subscribers: Mutex<Vec<mpsc::Sender<LifecycleEvent>>>,
}

impl EventBus {
    /// Creates an empty bus.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribes a new receiver. Events are cloned to each subscriber.
    #[must_use]
    pub fn subscribe(&self) -> mpsc::Receiver<LifecycleEvent> {
        let (tx, rx) = mpsc::channel();
        if let Ok(mut guard) = self.subscribers.lock() {
            guard.push(tx);
        }
        rx
    }

    /// Emits an event to all live subscribers and the tracing target.
    pub fn emit(&self, event: &LifecycleEvent) {
        if let Ok(payload) = serde_json::to_string(event) {
            tracing::info!(target: "containust.events", "{payload}");
        }
        let Ok(mut guard) = self.subscribers.lock() else {
            return;
        };
        guard.retain(|tx| tx.send(event.clone()).is_ok());
    }

    /// Emits a timed operation outcome.
    pub fn emit_operation(&self, op: OperationEmit) {
        self.emit(&LifecycleEvent::Operation {
            container_id: op.container_id.map(|id| id.as_str().to_string()),
            project: op.project,
            operation: op.operation,
            duration_ms: op.duration_ms,
            error_code: op.error_code.map(str::to_string),
        });
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn bus_delivers_operation_event() {
        let bus = EventBus::new();
        let rx = bus.subscribe();
        bus.emit_operation(OperationEmit {
            project: "proj".into(),
            operation: "deploy".into(),
            duration_ms: 12,
            container_id: None,
            error_code: None,
        });
        let event = rx.recv().unwrap();
        let LifecycleEvent::Operation {
            project,
            operation,
            duration_ms,
            error_code,
            ..
        } = event
        else {
            panic!("unexpected");
        };
        assert_eq!(project, "proj");
        assert_eq!(operation, "deploy");
        assert_eq!(duration_ms, 12);
        assert!(error_code.is_none());
    }

    #[test]
    fn bus_includes_error_code_on_failure() {
        let bus = EventBus::new();
        let rx = bus.subscribe();
        bus.emit_operation(OperationEmit {
            project: "p".into(),
            operation: "stop".into(),
            duration_ms: 3,
            container_id: None,
            error_code: Some("R001"),
        });
        let LifecycleEvent::Operation { error_code, .. } = rx.recv().unwrap() else {
            panic!("expected operation");
        };
        assert_eq!(error_code.as_deref(), Some("R001"));
    }
}
