//! Network tracing eBPF program.
//!
//! Defines the BPF program attached to socket/connect tracepoints.

/// Placeholder for the compiled eBPF network tracing program.
/// The actual BPF bytecode will be embedded at build time via `aya`.
pub const NETWORK_PROGRAM_NAME: &str = "containust_net_trace";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_program_name_is_non_empty() {
        assert!(!NETWORK_PROGRAM_NAME.is_empty());
    }

    #[test]
    fn network_program_name_contains_expected_prefix() {
        assert!(NETWORK_PROGRAM_NAME.starts_with("containust_"));
    }

    #[test]
    fn program_names_are_distinct() {
        use crate::programs::syscall::SYSCALL_PROGRAM_NAME;
        assert_ne!(NETWORK_PROGRAM_NAME, SYSCALL_PROGRAM_NAME);
    }
}
