//! Observability facade over optional eBPF probes.

/// Returns a short status string for `ctst doctor`.
#[must_use]
pub const fn ebpf_status() -> &'static str {
    containust_ebpf::availability_message()
}

/// Attempts to attach eBPF probes for `pid`.
///
/// # Errors
///
/// Returns an error when probes are unavailable or attach fails.
pub fn attach_ebpf(pid: u32) -> containust_common::error::Result<()> {
    containust_ebpf::attach(pid)
}

/// Detaches eBPF probes for `pid` (no-op when unavailable).
///
/// # Errors
///
/// Returns an error only when detach fails under an available probe build.
pub fn detach_ebpf(pid: u32) -> containust_common::error::Result<()> {
    containust_ebpf::detach(pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ebpf_status_is_non_empty() {
        assert!(!ebpf_status().is_empty());
    }
}
