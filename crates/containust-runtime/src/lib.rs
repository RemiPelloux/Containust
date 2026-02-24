//! # containust-runtime
//!
//! Container lifecycle management for the Containust runtime.
#![allow(clippy::todo)]
//!
//! Handles:
//! - **Container**: Core container struct and lifecycle operations.
//! - **Process**: Spawning processes inside isolated namespaces.
//! - **State**: State machine tracking (Created -> Running -> Stopped).
//! - **Exec**: Joining namespaces of running containers.
//! - **Metrics**: Real-time resource usage collection.

pub mod container;
pub mod exec;
pub mod metrics;
pub mod process;
pub mod state;
