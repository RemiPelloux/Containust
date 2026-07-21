# Containust — Sprint Tracker

> **Goal**: Ship Containust as a production-ready container runtime, lighter and more reliable than Docker.
> **Version**: `0.4.2`
> **Standards**: 90%+ test coverage for library crates, zero clippy warnings, `cargo deny` green, no banned patterns.

## Completed (evidence in `roadmap.md`)

- **Sprint 1 (`0.2.0`)** — runtime correctness, configuration propagation, deterministic CLI lifecycle.
- **Sprint 2 (`0.3.0`)** — project isolation, atomic schema-versioned state, reconciliation, concurrency safety.
- **Sprint 3 (`0.4.0`)** — content-addressed image import, digest-verified remote fetch, enforced offline operation; exit gate passed as a privileged Linux fixture.
- **Post–Sprint 3 (`0.4.1`)** — curated `preset://alpine` / `preset://busybox` catalog with pinned digests and `ctst images --presets`.
- **Performance pass (`0.4.2`)** — single-pass import hashing (pack, download, and staging are digested while written), in-place preset cache reuse (repeated `preset://` deploys cost one verification read, zero copies), and `crates/containust-image/tests/perf_regression.rs` gating 32 MiB import latency and staging-file hygiene. Measured on a 256 MiB image: cold `tar://` import 0.87 s → 0.50 s, warm re-import 0.61 s → 0.50 s, `file://` import 0.72 s → 0.65 s.

CI (`.github/workflows/ci.yml`), `deny.toml`, and the full deterministic test suite (490+ tests) are in place and green.

## Current sprint: Sprint 4 — Security hardening (`0.5.0`)

Validate and enforce the security model rather than only exposing security-shaped options.

- [ ] **S4.1 Rootfs safety.** Reject path traversal and unsafe tar entries during extraction; never follow symlinks outside the extraction root.
- [ ] **S4.2 Capability policy.** Minimal default capability set, allowlist validation, privileged Linux fixture proving the effective drop.
- [ ] **S4.3 Namespace policy.** Explicit user/PID/mount/UTS/IPC/network namespace choices in runtime config; unsupported combinations fail closed.
- [ ] **S4.4 Mount and volume policy.** Validate source/target paths, prevent host escape, enforce read-only mounts, clean unmount on teardown.
- [ ] **S4.5 Resource limits.** Validate CPU/memory ranges; fail closed when a requested cgroup limit cannot be applied.
- [ ] **S4.6 Secret handling.** Keep secrets out of state, logs, debug output, and generated plans; add redaction tests.
- [ ] **S4.7 Threat-model review.** Update the threat model; run `cargo deny`, advisory, license, and unsafe-code reviews in CI.

**Exit gate:** security-sensitive inputs have negative tests, privileged tests pass on the supported Linux matrix, and an independent review signs off on the threat model.

## Deferred performance backlog (Sprint 7)

- Lazy layers (FUSE), parallel multi-layer extraction, startup caching, syscall overhead benchmarks, published regression budgets on reference machines.
