# Containust — Sprint Tracker

> **Goal**: Ship Containust as a production-ready container runtime, lighter and more reliable than Docker.
> **Version**: `1.1.0` (Sprint 10 complete)
> **Standards**: 90%+ test coverage for library crates, zero clippy warnings, `cargo deny` green, no banned patterns.

## Completed

- **Sprint 1–5 (`0.2.0`–`0.6.0`)** — runtime, isolation, images, security, cross-platform VM backend.
- **Sprint 6 (`0.7.0`)** — events, metrics, TUI, eBPF gates, doctor, error codes.
- **Sprint 7 (`0.8.0`)** — versioning, release metadata, packaging deferrals, CI docs/coverage, perf, runbooks.
- **Sprint 8 (`0.9.0-beta.1`)** — feature freeze, compat matrix, upgrade rehearsal, beta RC.
- **Sprint 9 (`1.0.0`)** — GA checklist, support policy, security/perf sign-off docs, GA tag.
- **Patches `1.0.1`–`1.0.5`** — Windows/QEMU CI, TCG accel, VM agent reachability, Windows volumes.

## Sprint 10 — Production-usable v1 (`1.1.0`) — complete

Tracked in `roadmap.md` (P10.1–P10.18) and gated by `docs/PROD_CHECKLIST.md`.

- **Wave 1** — ✅ trackers/PROD_CHECKLIST, privileged Linux CI job.
- **Wave 2** — ✅ OCI registry pull (`oci://`), auth, catalog import, `ctst pull`.
- **Wave 3** — ✅ `ports` / `EXPOSE` (identity mapping), `restart` policies, and
  `healthcheck` probes enforced through daemonless reconciliation; state schema 3.
- **Wave 4** — ✅ Homebrew/deb/RPM/winget packaging, cosign-signed checksums,
  `v1.1.0` tagged on green CI (Linux, macOS, Windows, QEMU smoke, privileged Linux).

## Next — Sprint 11 (`1.2.0`)

Isolation depth + networking. See `roadmap.md` Sprint 11 (P11.1–P11.9).

- User + PID namespaces on the Linux spawn path.
- Linux veth/NAT port remapping (`EXPOSE host:container` with differing ports).
- Named networks + DNS foundations for CONNECT.
- Homebrew tap automation, winget submission, optional OCI provenance checks.

## Deferred (Sprint 12+)

- Rolling updates / declarative plan apply diffs.
- Volume drivers, snapshots, encrypted storage.
- Remote execution / orchestration.
- Apple notarization / Windows Authenticode.
