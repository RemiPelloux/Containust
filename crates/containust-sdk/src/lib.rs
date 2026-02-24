//! # containust-sdk
//!
//! Public SDK for using Containust as a Rust library.
#![allow(clippy::todo)]
//!
//! Provides three main entry points:
//! - [`ContainerBuilder`](builder::ContainerBuilder): Fluent API for configuring and launching containers.
//! - [`GraphResolver`](graph_resolver::GraphResolver): Validates and resolves component dependency graphs.
//! - [`EventListener`](event::EventListener): Subscribes to container lifecycle events for monitoring.
//!
//! # Example
//!
//! ```rust,no_run
//! use containust_sdk::builder::ContainerBuilder;
//!
//! let container = ContainerBuilder::new("my-app")
//!     .image("file:///opt/images/alpine")
//!     .memory_limit(128 * 1024 * 1024)
//!     .build();
//! ```

pub mod builder;
pub mod event;
pub mod graph_resolver;
