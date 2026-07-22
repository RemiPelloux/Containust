# Containust — Sprint Tracker

> **Goal**: Ship Containust as a production-ready container runtime, lighter and more reliable than Docker.
> **Version**: `0.8.0`
> **Standards**: 90%+ test coverage for library crates, zero clippy warnings, `cargo deny` green, no banned patterns.

## Completed

- **Sprint 1–5 (`0.2.0`–`0.6.0`)** — runtime, isolation, images, security, cross-platform VM backend.
- **Sprint 6 (`0.7.0`)** — events, metrics availability semantics, `ctst ps --tui`, eBPF attach/detach gates, `ctst doctor`, CLI error codes.
- **Sprint 7 (`0.8.0`)** — versioning contract, release metadata/checksums, packaging deferrals, CI docs+coverage, perf budgets, runbooks.

## Current sprint: Sprint 8 → `0.9.0-beta`

- [x] **B8.1** Feature freeze policy (`docs/FEATURE_FREEZE.md`).
- [x] **B8.2** Compatibility matrix tests.
- [ ] **B8.3** Upgrade rehearsal.
- [ ] **B8.4** Publish `0.9.0-beta` RC.

Sprint 9 (`1.0.0`): GA.

## Deferred

- PID / user namespace wiring on the spawn path (double-fork + uid maps).
- OCI registry pull for Hub-style names.
- Code signing / Homebrew / deb / RPM / winget (see `docs/PACKAGING.md`).
