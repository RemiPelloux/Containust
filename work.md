# Containust — Sprint Tracker

> **Goal**: Ship Containust as a production-ready container runtime, lighter and more reliable than Docker.
> **Version**: `0.6.0`
> **Standards**: 90%+ test coverage for library crates, zero clippy warnings, `cargo deny` green, no banned patterns.

## Completed

- **Sprint 1 (`0.2.0`)** — runtime correctness, configuration propagation, deterministic CLI lifecycle.
- **Sprint 2 (`0.3.0`)** — project isolation, atomic schema-versioned state, reconciliation, concurrency safety.
- **Sprint 3 (`0.4.0`)** — content-addressed image import, digest-verified remote fetch, enforced offline operation.
- **Post–Sprint 3 (`0.4.1`)** — curated `preset://alpine` / `preset://busybox`.
- **Performance pass (`0.4.2`)** — single-pass import hashing, in-place preset cache reuse, perf regression gates.
- **Sprint 4 (`0.5.0`)** — security hardening (S4.1–S4.7). See `docs/THREAT_MODEL.md` and `roadmap.md`.
- **Sprint 5 (`0.6.0`)** — cross-platform VM backend (V5.1–V5.6).

## Current sprint: Sprint 6 — Observability and operator experience (`0.7.0`)

Make failures diagnosable during repeated operational use.

- [ ] **O6.1 Structured events.** Emit lifecycle events with container ID, project, operation, duration, and error code.
- [ ] **O6.2 Metrics correctness.** Validate CPU, memory, I/O, and process metrics against a known workload; define zero/unavailable semantics.
- [ ] **O6.3 TUI integration.** Wire `ctst ps --tui` to the real engine and support refresh, selection, logs, and quit behavior.
- [ ] **O6.4 eBPF lifecycle.** Implement feature-gated load/attach/detach, capability checks, and graceful degradation when unsupported.
- [ ] **O6.5 Diagnostics.** Add `ctst doctor` for platform, cgroups, namespace, mount, QEMU, cache, and permissions checks.
- [ ] **O6.6 Error UX.** Standardize exit codes and include a remediation hint for every user-facing runtime error.

**Exit gate:** a failure can be diagnosed from CLI output and logs without reading source code; TUI and eBPF are clearly reported as unavailable when prerequisites are missing.

## Deferred

- PID / user namespace wiring on the spawn path (double-fork + uid maps) — currently fail closed if requested.
- Lazy FUSE layers, parallel extraction, syscall benchmarks (Sprint 7).
- OCI registry pull for `preset://node` / Hub names (later backlog).
