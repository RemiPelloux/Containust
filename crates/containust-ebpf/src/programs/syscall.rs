//! Syscall tracepoint eBPF program.
//!
//! Defines the BPF program attached to `sys_enter` tracepoints.

/// Placeholder for the compiled eBPF syscall tracing program.
/// The actual BPF bytecode will be embedded at build time via `aya`.
pub const SYSCALL_PROGRAM_NAME: &str = "containust_syscall_trace";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syscall_program_name_is_non_empty() {
        assert!(!SYSCALL_PROGRAM_NAME.is_empty());
    }

    #[test]
    fn syscall_program_name_contains_expected_prefix() {
        assert!(SYSCALL_PROGRAM_NAME.starts_with("containust_"));
    }
}
