//! # containust-tui
//!
//! Interactive terminal dashboard for monitoring Containust containers.
//!
//! Built with `ratatui` and `crossterm`, providing:
//! - Real-time container status overview.
//! - Per-container resource metrics (CPU, memory, I/O).
//! - eBPF trace viewer for syscalls, file access, and network events.

pub mod app;
pub mod event;
pub mod ui;
