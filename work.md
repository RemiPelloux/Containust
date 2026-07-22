# Containust — Sprint Tracker

> **Goal**: Ship Containust as a production-ready container runtime, lighter and more reliable than Docker.
> **Version**: `0.5.0`
> **Standards**: 90%+ test coverage for library crates, zero clippy warnings, `cargo deny` green, no banned patterns.

## Completed

- **Sprint 1 (`0.2.0`)** — runtime correctness, configuration propagation, deterministic CLI lifecycle.
- **Sprint 2 (`0.3.0`)** — project isolation, atomic schema-versioned state, reconciliation, concurrency safety.
- **Sprint 3 (`0.4.0`)** — content-addressed image import, digest-verified remote fetch, enforced offline operation.
- **Post–Sprint 3 (`0.4.1`)** — curated `preset://alpine` / `preset://busybox`.
- **Performance pass (`0.4.2`)** — single-pass import hashing, in-place preset cache reuse, perf regression gates.
- **Sprint 4 (`0.5.0`)** — security hardening (S4.1–S4.7). See `docs/THREAT_MODEL.md` and `roadmap.md`.

## Current sprint: Sprint 5 — Cross-platform VM backend (`0.6.0`)

Make macOS and Windows execution a supported workflow with deterministic assets and lifecycle behavior.

- [x] **V5.1 VM asset manifest.** Pinned Alpine 3.21.7 netboot kernel/initramfs URLs + SHA-256 per arch (`backend/vm/assets.rs`); cache hits re-verified; corrupt blobs re-downloaded fail-closed.
- [ ] **V5.2 Asset cache.** Resumable downloads, cache locking, clear offline failures.
- [ ] **V5.3 VM lifecycle.** Idempotent start/stop, readiness, graceful shutdown, stale-process recovery.
- [ ] **V5.4 RPC contract.** Versioned agent protocol, request IDs, timeouts, bounded payloads.
- [ ] **V5.5 Port forwarding.** Ownership/teardown of forwarded ports; reject collisions before boot.
- [ ] **V5.6 Cross-platform CI.** Compile/unit tests on macOS and Windows; QEMU smoke on one hosted platform.

**Exit gate:** a documented macOS and Windows quickstart can build, boot, run, inspect, and stop a local image without manual VM cleanup.

## Deferred

- PID / user namespace wiring on the spawn path (double-fork + uid maps) — currently fail closed if requested.
- Lazy FUSE layers, parallel extraction, syscall benchmarks (Sprint 7).
- OCI registry pull for `preset://node` / Hub names (later backlog).
