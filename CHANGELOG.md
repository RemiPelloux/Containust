# Changelog

All notable changes to Containust are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Compatibility guarantees for `.ctst`, `state.json`, and the SDK are described in
[`docs/VERSIONING.md`](docs/VERSIONING.md).

## [Unreleased]

## [1.0.2] — 2026-07-22

### Fixed

- `cargo fmt` on VM boot timeout helpers; clippy `too_many_lines` in sprint3 gate.

## [1.0.1] — 2026-07-22

### Fixed

- Windows: compare process `HANDLE` with `.is_null()` so release/CI builds compile.
- Linux clippy: metrics Option handling and redundant `pub(crate)` on private mounts helper.
- VM boot wait default raised to 180s (override with `CONTAINUST_VM_BOOT_TIMEOUT_SECS`) for cold CI boots.

## [1.0.0] — 2026-07-22

### Added

- GA support policy (`docs/SUPPORT_POLICY.md`) and release checklist (`docs/GA_CHECKLIST.md`).

### Changed

- First general-availability release; SemVer stability policy applies to the SDK and documented surfaces.

## [0.9.0-beta.1] — 2026-07-22

### Added

- Feature freeze policy (`docs/FEATURE_FREEZE.md`) and compatibility matrix tests (B8.1/B8.2).
- Upgrade/rollback rehearsal tests and runbook checklist (B8.3).
- Beta RC install matrix (`docs/BETA_RC.md`) (B8.4).

## [0.8.0] — 2026-07-22

### Added

- Workspace versioning and compatibility documentation (`docs/VERSIONING.md`).
- Shared `STATE_SCHEMA_VERSION` constant in `containust-common`.
- Release build metadata (`git=` / `built=`) via CLI `build.rs`; archives include `build-info.json`.
- Packaging deferrals (`docs/PACKAGING.md`), operator runbooks (`docs/RUNBOOKS.md`), performance budgets (`docs/PERFORMANCE.md`).
- CI docs + library coverage jobs; compose parse/resolve regression gates.

## [0.7.0] — 2026-07-22

### Added

- Structured lifecycle `EventBus` / operation events with SDK subscribe API.
- Metrics availability semantics (`Available` / `Unavailable` / `Missing`) for CPU, memory, and I/O.
- Interactive `ctst ps --tui` dashboard driven by the engine listing.
- Feature/OS-gated eBPF attach/detach lifecycle with graceful degradation.
- `ctst doctor` for platform, backend, cache, assets, offline, and cgroup readiness.
- Stable CLI error codes with remediation hints (`containust_common::codes`).

### Changed

- Non-Linux metric zeros report unavailable rather than idle.

## [0.6.0] — 2026-07-22

### Added

- Cross-platform VM backend: pinned Alpine assets, resumable downloads, pidfile lifecycle.
- Versioned VM agent RPC (`v=1`) with timeouts and payload caps.
- Port probe/ownership for forwarded ports.
- macOS/Windows CI jobs and QEMU smoke on macOS.

## [0.5.0] — 2026-07

### Added

- Security hardening sprint deliverables (capabilities, isolation, offline, supply chain).

## [0.4.0] — 2026-07

### Added

- Image catalog, import, and offline-first layer handling.

## [0.3.0] — 2026-07

### Added

- Project isolation, atomic state, reconciliation, concurrency safety.

## [0.2.0] — 2026-07

### Added

- Core runtime, namespaces/cgroups foundations, CLI/SDK scaffolding.
