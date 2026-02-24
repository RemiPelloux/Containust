//! Syscall tracepoint eBPF program.
//!
//! Defines the BPF program attached to `sys_enter` tracepoints.

/// Placeholder for the compiled eBPF syscall tracing program.
/// The actual BPF bytecode will be embedded at build time via `aya`.
pub const SYSCALL_PROGRAM_NAME: &str = "containust_syscall_trace";
