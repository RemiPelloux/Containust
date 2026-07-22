# Containust — Sprint Tracker

> **Goal**: Ship Containust as a production-ready container runtime, lighter and more reliable than Docker.
> **Version**: `0.7.0`
> **Standards**: 90%+ test coverage for library crates, zero clippy warnings, `cargo deny` green, no banned patterns.

## Completed

- **Sprint 1–5 (`0.2.0`–`0.6.0`)** — runtime, isolation, images, security, cross-platform VM backend.
- **Sprint 6 (`0.7.0`)** — events, metrics availability semantics, `ctst ps --tui`, eBPF attach/detach gates, `ctst doctor`, CLI error codes.

## Current sprint: Sprint 7 → `0.8.0`

Release readiness: packaging, CI gates, coverage, benchmarks, runbooks.

Sprint 8 (`0.9.0-beta`): feature freeze + upgrade rehearsal.  
Sprint 9 (`1.0.0`): GA.

## Deferred

- PID / user namespace wiring on the spawn path (double-fork + uid maps).
- OCI registry pull for Hub-style names.
