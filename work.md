# Containust — Sprint Tracker

> **Goal**: Ship Containust as a production-ready container runtime, lighter and more reliable than Docker.
> **Version**: `0.7.0`
> **Standards**: 90%+ test coverage for library crates, zero clippy warnings, `cargo deny` green, no banned patterns.

## Completed

- **Sprint 1–5 (`0.2.0`–`0.6.0`)** — runtime, isolation, images, security, cross-platform VM backend.
- **Sprint 6 (`0.7.0`)** — structured events, `ctst doctor`, CLI error codes + remediation hints (O6.1/O6.5/O6.6 core).

## Current sprint: Sprint 6 remainder → Sprint 7

Still open from Sprint 6:

- [ ] **O6.2 Metrics correctness.** Validate CPU, memory, I/O, and process metrics; define zero/unavailable semantics.
- [ ] **O6.3 TUI integration.** Wire `ctst ps --tui` to the real engine.
- [ ] **O6.4 eBPF lifecycle.** Feature-gated load/attach/detach with graceful degradation.

Then Sprint 7 (`0.8.0`): packaging, CI gates, coverage, benchmarks, runbooks.  
Sprint 8 (`0.9.0-beta`): feature freeze + upgrade rehearsal.  
Sprint 9 (`1.0.0`): GA.

## Deferred

- PID / user namespace wiring on the spawn path (double-fork + uid maps).
- OCI registry pull for Hub-style names.
