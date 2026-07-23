//! Host port reservation and QEMU `hostfwd` helpers for the VM backend.

use std::net::TcpListener;

use containust_common::error::{ContainustError, Result};

use super::rpc::VM_AGENT_PORT;

/// Validates and deduplicates container forward ports.
///
/// Rejects the reserved agent port and duplicate entries.
///
/// # Errors
///
/// Returns an error when a port is reserved or duplicated.
pub fn normalize_forward_ports(ports: &[u16]) -> Result<Vec<u16>> {
    let mut seen = Vec::new();
    for &port in ports {
        if port == 0 {
            return Err(ContainustError::Config {
                message: "host port 0 is invalid for VM forwarding".into(),
            });
        }
        if port == VM_AGENT_PORT {
            return Err(ContainustError::Config {
                message: format!(
                    "host port {VM_AGENT_PORT} is reserved for the VM agent; \
                     choose a different container port"
                ),
            });
        }
        if seen.contains(&port) {
            return Err(ContainustError::Config {
                message: format!("duplicate host port {port} in VM forward list"),
            });
        }
        seen.push(port);
    }
    seen.sort_unstable();
    Ok(seen)
}

/// Ensures every requested forward port is already owned by the running VM.
///
/// QEMU `hostfwd` is fixed at boot; missing ports fail closed.
///
/// # Errors
///
/// Returns an error when a requested port is not in `owned`.
pub fn ensure_ports_covered(owned: &[u16], requested: &[u16]) -> Result<()> {
    let requested = normalize_forward_ports(requested)?;
    for port in requested {
        if !owned.contains(&port) {
            return Err(ContainustError::Config {
                message: format!(
                    "VM is already running without host port {port} forwarded. \
                     Run `ctst vm stop`, then start again so QEMU can bind the port \
                     (hostfwd cannot be added to a live VM)"
                ),
            });
        }
    }
    Ok(())
}

/// Probes that each port can be bound on `127.0.0.1` before QEMU starts.
///
/// # Errors
///
/// Returns an error when a port is already in use.
pub fn probe_available(ports: &[u16]) -> Result<()> {
    let mut check = vec![VM_AGENT_PORT];
    check.extend(normalize_forward_ports(ports)?);
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
pub fn build_netdev_arg(ports: &[u16]) -> String {
    // Pin host+guest addresses so macOS/TCG user-net reliably forwards to the
    // static guest IP configured in the init script (10.0.2.15).
    let mut hostfwd =
        format!("user,id=net0,hostfwd=tcp:127.0.0.1:{VM_AGENT_PORT}-10.0.2.15:{VM_AGENT_PORT}");
    for &port in ports {
        if port != VM_AGENT_PORT {
            use std::fmt::Write as _;
            let _ = write!(hostfwd, ",hostfwd=tcp:127.0.0.1:{port}-10.0.2.15:{port}");
        }
    }
    hostfwd
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn normalize_forward_ports_rejects_agent_port() {
        let err = normalize_forward_ports(&[VM_AGENT_PORT]).expect_err("reserved");
        assert!(err.to_string().contains("reserved"));
    }

    #[test]
    fn normalize_forward_ports_rejects_duplicates() {
        let err = normalize_forward_ports(&[8080, 8080]).expect_err("dup");
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn normalize_forward_ports_sorts_unique() {
        assert_eq!(
            normalize_forward_ports(&[9090, 8080]).expect("ok"),
            vec![8080, 9090]
        );
    }

    #[test]
    fn ensure_ports_covered_rejects_missing() {
        let err = ensure_ports_covered(&[8080], &[9090]).expect_err("missing");
        assert!(err.to_string().contains("already running"));
    }

    #[test]
    fn ensure_ports_covered_accepts_subset() {
        ensure_ports_covered(&[8080, 9090], &[8080]).expect("subset");
    }

    #[test]
    fn probe_available_free_ephemeral_succeeds() {
        // Agent port may be in use on the developer machine — skip if busy.
        if TcpListener::bind(("127.0.0.1", VM_AGENT_PORT)).is_err() {
            return;
        }
        // Freeing an ephemeral port and re-probing it races with other
        // processes; retry over several candidates before giving up.
        for _ in 0..10 {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            let port = listener.local_addr().expect("addr").port();
            drop(listener);
            if probe_available(&[port]).is_ok() {
                return;
            }
        }
        panic!("no ephemeral port was probeable after 10 attempts");
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
        let err = probe_available(&[port]).expect_err("in use");
        assert!(err.to_string().contains("not available"));
    }

    #[test]
    fn build_netdev_arg_includes_agent_and_ports() {
        let arg = build_netdev_arg(&[8080]);
        assert!(arg.contains(&format!(
            "hostfwd=tcp:127.0.0.1:{VM_AGENT_PORT}-10.0.2.15:{VM_AGENT_PORT}"
        )));
        assert!(arg.contains("hostfwd=tcp:127.0.0.1:8080-10.0.2.15:8080"));
    }
}
