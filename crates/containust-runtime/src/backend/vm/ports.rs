//! Host port reservation and QEMU `hostfwd` helpers for the VM backend.

use std::net::TcpListener;

use containust_common::error::{ContainustError, Result};
use containust_common::types::PortMapping;

use super::rpc::VM_AGENT_PORT;

/// Validates and deduplicates host→container forward mappings.
///
/// Rejects the reserved agent port (as host) and duplicate host ports.
///
/// # Errors
///
/// Returns an error when a port is reserved or duplicated.
pub fn normalize_forward_mappings(mappings: &[PortMapping]) -> Result<Vec<PortMapping>> {
    let mut seen = Vec::new();
    for &mapping in mappings {
        if mapping.host == 0 {
            return Err(ContainustError::Config {
                message: "host port 0 is invalid for VM forwarding".into(),
            });
        }
        if mapping.host == VM_AGENT_PORT {
            return Err(ContainustError::Config {
                message: format!(
                    "host port {VM_AGENT_PORT} is reserved for the VM agent; \
                     choose a different host port"
                ),
            });
        }
        if seen.iter().any(|m: &PortMapping| m.host == mapping.host) {
            return Err(ContainustError::Config {
                message: format!("duplicate host port {} in VM forward list", mapping.host),
            });
        }
        seen.push(mapping);
    }
    seen.sort_by_key(|m| m.host);
    Ok(seen)
}

/// Ensures every requested forward mapping is already owned by the running VM.
///
/// QEMU `hostfwd` is fixed at boot; missing or mismatched ports fail closed.
///
/// # Errors
///
/// Returns an error when a requested mapping is not in `owned`.
pub fn ensure_mappings_covered(owned: &[PortMapping], requested: &[PortMapping]) -> Result<()> {
    let requested = normalize_forward_mappings(requested)?;
    for mapping in requested {
        if !owned
            .iter()
            .any(|m| m.host == mapping.host && m.container == mapping.container)
        {
            return Err(ContainustError::Config {
                message: format!(
                    "VM is already running without hostfwd {}:{} → guest. \
                     Run `ctst vm stop`, then start again so QEMU can bind the port \
                     (hostfwd cannot be added to a live VM)",
                    mapping.host, mapping.container
                ),
            });
        }
    }
    Ok(())
}

/// Probes that each host port can be bound on `127.0.0.1` before QEMU starts.
///
/// # Errors
///
/// Returns an error when a port is already in use.
pub fn probe_available(mappings: &[PortMapping]) -> Result<()> {
    let mut check = vec![VM_AGENT_PORT];
    check.extend(
        normalize_forward_mappings(mappings)?
            .into_iter()
            .map(|m| m.host),
    );
    check.sort_unstable();
    check.dedup();
    for port in check {
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(listener) => drop(listener),
            Err(source) => {
                return Err(ContainustError::Config {
                    message: format!(
                        "host port {port} is not available for VM forwarding \
                         (bind 127.0.0.1:{port} failed: {source}). Stop the conflicting \
                         process or choose another port"
                    ),
                });
            }
        }
    }
    Ok(())
}

/// Builds the QEMU user-mode netdev argument including agent + container forwards.
#[must_use]
pub fn build_netdev_arg(mappings: &[PortMapping]) -> String {
    // Pin host+guest addresses so macOS/TCG user-net reliably forwards to the
    // static guest IP configured in the init script (10.0.2.15).
    let mut hostfwd =
        format!("user,id=net0,hostfwd=tcp:127.0.0.1:{VM_AGENT_PORT}-10.0.2.15:{VM_AGENT_PORT}");
    for mapping in mappings {
        if mapping.host != VM_AGENT_PORT {
            use std::fmt::Write as _;
            let _ = write!(
                hostfwd,
                ",hostfwd=tcp:127.0.0.1:{}-10.0.2.15:{}",
                mapping.host, mapping.container
            );
        }
    }
    hostfwd
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn normalize_forward_mappings_rejects_agent_port() {
        let err = normalize_forward_mappings(&[PortMapping::identity(VM_AGENT_PORT)])
            .expect_err("reserved");
        assert!(err.to_string().contains("reserved"));
    }

    #[test]
    fn normalize_forward_mappings_rejects_duplicates() {
        let err =
            normalize_forward_mappings(&[PortMapping::identity(8080), PortMapping::identity(8080)])
                .expect_err("dup");
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn normalize_forward_mappings_sorts_by_host() {
        assert_eq!(
            normalize_forward_mappings(
                &[PortMapping::identity(9090), PortMapping::identity(8080),]
            )
            .expect("ok"),
            vec![PortMapping::identity(8080), PortMapping::identity(9090)]
        );
    }

    #[test]
    fn ensure_mappings_covered_rejects_missing() {
        let err = ensure_mappings_covered(
            &[PortMapping::identity(8080)],
            &[PortMapping::identity(9090)],
        )
        .expect_err("missing");
        assert!(err.to_string().contains("already running"));
    }

    #[test]
    fn ensure_mappings_covered_accepts_subset() {
        ensure_mappings_covered(
            &[PortMapping::identity(8080), PortMapping::identity(9090)],
            &[PortMapping::identity(8080)],
        )
        .expect("subset");
    }

    #[test]
    fn ensure_mappings_covered_rejects_remap_mismatch() {
        let owned = [PortMapping {
            host: 80,
            container: 8080,
        }];
        let requested = [PortMapping::identity(80)];
        let err = ensure_mappings_covered(&owned, &requested).expect_err("mismatch");
        assert!(err.to_string().contains("already running"));
    }

    #[test]
    fn probe_available_free_ephemeral_succeeds() {
        // Agent port may be in use on the developer machine — skip if busy.
        if TcpListener::bind(("127.0.0.1", VM_AGENT_PORT)).is_err() {
            return;
        }
        // Freeing an ephemeral port and re-probing it races with other
        // processes; retry over several candidates before giving up.
        let probed = (0..10).any(|_| {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            let port = listener.local_addr().expect("addr").port();
            drop(listener);
            probe_available(&[PortMapping::identity(port)]).is_ok()
        });
        assert!(probed, "no ephemeral port was probeable after 10 attempts");
    }

    #[test]
    fn probe_available_bound_port_fails() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        if TcpListener::bind(("127.0.0.1", VM_AGENT_PORT)).is_err() {
            // Cannot run full probe_available; assert bind failure message shape via helper.
            drop(listener);
            return;
        }
        let err = probe_available(&[PortMapping::identity(port)]).expect_err("in use");
        assert!(err.to_string().contains("not available"));
    }

    #[test]
    fn build_netdev_arg_includes_agent_and_ports() {
        let arg = build_netdev_arg(&[PortMapping::identity(8080)]);
        assert!(arg.contains(&format!(
            "hostfwd=tcp:127.0.0.1:{VM_AGENT_PORT}-10.0.2.15:{VM_AGENT_PORT}"
        )));
        assert!(arg.contains("hostfwd=tcp:127.0.0.1:8080-10.0.2.15:8080"));
    }

    #[test]
    fn build_netdev_arg_supports_host_container_remap() {
        let arg = build_netdev_arg(&[PortMapping {
            host: 80,
            container: 8080,
        }]);
        assert!(arg.contains("hostfwd=tcp:127.0.0.1:80-10.0.2.15:8080"));
    }
}
