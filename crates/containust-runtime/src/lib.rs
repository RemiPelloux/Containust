//! Container lifecycle management for the Containust runtime.

#![allow(unsafe_code, clippy::print_stderr)]
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

pub mod backend;
pub mod container;
pub mod engine;
pub mod events;
pub mod exec;
pub mod logs;
pub mod metrics;
pub mod network;
pub mod observe;
#[cfg(target_os = "linux")]
pub mod port_forward;
pub mod process;
mod process_mounts;
#[cfg(target_os = "linux")]
mod process_spawn;
#[cfg(target_os = "linux")]
mod process_spawn_io;
pub mod state;
pub mod supervise;
pub mod volume;
