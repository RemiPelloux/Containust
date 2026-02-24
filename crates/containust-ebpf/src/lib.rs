//! # containust-ebpf
//!
//! eBPF-based observability for the Containust runtime.
//!
//! Provides real-time, low-overhead monitoring of container activity:
//! - **Syscall tracing**: Track system calls made by container processes.
//! - **File monitoring**: Observe file opens and modifications.
//! - **Network monitoring**: Track socket creation and network connections.
//!
//! The `ebpf` feature flag must be enabled and the host must support
//! BPF for these capabilities to be available.

pub mod file_monitor;
pub mod net_monitor;
pub mod programs;
pub mod tracer;
