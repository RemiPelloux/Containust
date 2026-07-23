# Containust — Sprint Tracker

> **Goal**: Ship Containust as a production-ready container runtime, lighter and more reliable than Docker.
> **Version**: `1.2.0` (Sprint 11 complete)
> **License**: Containust Commercial License (source-available; see `COMMERCIAL.md`)
> **Standards**: 90%+ test coverage for library crates, zero clippy warnings, `cargo deny` green, no banned patterns.

## Completed

- **Sprint 1–5 (`0.2.0`–`0.6.0`)** — runtime, isolation, images, security, cross-platform VM backend.
- **Sprint 6 (`0.7.0`)** — events, metrics, TUI, eBPF gates, doctor, error codes.
- **Sprint 7 (`0.8.0`)** — versioning, release metadata, packaging deferrals, CI docs/coverage, perf, runbooks.
- **Sprint 8 (`0.9.0-beta.1`)** — feature freeze, compat matrix, upgrade rehearsal, beta RC.
- **Sprint 9 (`1.0.0`)** — GA checklist, support policy, security/perf sign-off docs, GA tag.
- **Patches `1.0.1`–`1.0.5`** — Windows/QEMU CI, TCG accel, VM agent reachability, Windows volumes.
- **Sprint 10 (`1.1.0`)** — OCI pull, ports/restart/healthcheck, privileged CI, packaging.
- **Sprint 11 (`1.2.0`)** — user/PID ns, port remap, named networks, DNS foundations,
  Homebrew/winget bump automation, `ctst pull --require-provenance`, commercial license.

## Next (Sprint 12+)

- Rolling updates / declarative plan apply diffs.
- Volume drivers, snapshots, encrypted storage.
- Remote execution / orchestration.
- Apple notarization / Windows Authenticode.
- Optional: `homebrew-containust` tap repo + winget-pkgs PR for `1.2.0`.
