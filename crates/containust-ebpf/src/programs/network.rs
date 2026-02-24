//! Network tracing eBPF program.
//!
//! Defines the BPF program attached to socket/connect tracepoints.

/// Placeholder for the compiled eBPF network tracing program.
/// The actual BPF bytecode will be embedded at build time via `aya`.
pub const NETWORK_PROGRAM_NAME: &str = "containust_net_trace";
