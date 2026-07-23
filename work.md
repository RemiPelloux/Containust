# Containust — Sprint Tracker

> **Goal**: Ship Containust as a production-ready container runtime, lighter and more reliable than Docker.
> **Version**: `1.1.0` (Sprint 11 Waves 1–3 on `main` → tag `1.2.0`)
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

## Sprint 11 — Isolation depth + networking (`1.2.0`) — Waves 1–3 complete

See `roadmap.md` (P11.1–P11.9). Docs: `site/docs/`, `docs/HowToUse.md`.

- **Wave 1** — ✅ user/PID namespaces default-on; READY handshake; proc anchor; privileged fixtures green.
- **Wave 2** — ✅ `EXPOSE` remapping; named networks; `/etc/hosts` DNS; state schema 4.
- **Wave 3** — ✅ Homebrew tap automation + bump script; winget bump + submit docs;
  `ctst pull --require-provenance` (cosign fail-closed).
- **Docs / license** — ✅ landing + HTML docs; commercial license + deny SPDX.

### Cut `v1.2.0`

1. Bump workspace version `1.1.0` → `1.2.0`, cut CHANGELOG.
2. Tag `v1.2.0` on green CI (triggers release + packaging-bump PR).
3. Create `RemiPelloux/homebrew-containust` + `HOMEBREW_TAP_TOKEN` (optional).
4. After bump PR: `wingetcreate submit` for Windows.

## Deferred (Sprint 12+)

- Rolling updates / declarative plan apply diffs.
- Volume drivers, snapshots, encrypted storage.
- Remote execution / orchestration.
- Apple notarization / Windows Authenticode.
