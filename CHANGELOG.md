# Changelog

All notable changes to Containust are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Compatibility guarantees for `.ctst`, `state.json`, and the SDK are described in
[`docs/VERSIONING.md`](docs/VERSIONING.md).

## [Unreleased]

### Added

- Workspace versioning and compatibility documentation (`docs/VERSIONING.md`).
- Shared `STATE_SCHEMA_VERSION` constant in `containust-common`.

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
