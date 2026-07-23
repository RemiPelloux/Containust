# Changelog

All notable changes to Containust are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Compatibility guarantees for `.ctst`, `state.json`, and the SDK are described in
[`docs/VERSIONING.md`](docs/VERSIONING.md).

## [Unreleased]

### Added

- Product landing page in [`site/`](site/) (open `site/index.html`).
- HTML documentation site in [`site/docs/`](site/docs/) (get started, CLI,
  language, migrate).
- Docs map [`docs/README.md`](docs/README.md); process checklists under `docs/process/`.
- Operator guide [`docs/HowToUse.md`](docs/HowToUse.md) — install, first pull/run,
  everyday commands, ports, offline, troubleshooting.
- Linux spawn path for **user + PID namespaces** (pipe-synced uid/gid maps and
  post-`CLONE_NEWPID` double-fork so container init is PID 1); **default-on**.
- **`EXPOSE host:container` remapping** on Linux (userspace TCP forwarder into
  the container/shared netns) and on the VM backend (QEMU `hostfwd` remap).
- **Named networks** (`host` / `none` / `bridge` / custom) with persisted
  shared netns; peer names in `/etc/hosts` for `CONNECT` resolution.
- State schema **4**: `port_mappings`, `network`, `forwarder_pids`.
- **Homebrew tap automation** (`scripts/bump_packaging.sh`, release
  `packaging-bump` job, `packaging/homebrew-tap/README.md`).
- **winget manifest bump** on every `v*` release (submit via
  `packaging/winget/README.md`).
- **`ctst pull --require-provenance`** — fail-closed `cosign verify` before
  layer download (`CONTAINUST_REQUIRE_PROVENANCE`, identity env regexps).

### Changed

- README, roadmap, support policy, and sprint tracker updated for Sprint 11
  Waves 1–3 (`1.2.0` track).
- **License:** switched from MIT/Apache-2.0 dual licensing to the proprietary
  [Containust Commercial License](LICENSE). Source remains viewable for
  evaluation; production use and redistribution require a paid license
  ([COMMERCIAL.md](COMMERCIAL.md)). Prior MIT/Apache releases are unchanged.

## [1.1.0] — 2026-07-23

### Added

- **OCI registry pull** (`ctst pull`, `oci://` scheme): manifest index →
  platform manifest → digest-verified layer blobs, imported into the local
  content-addressed catalog as `image://name@sha256:...`. Digest pin required
  by default; `--offline` rejects registries before connecting.
- Registry auth via `CONTAINUST_REGISTRY_TOKEN`, `CONTAINUST_REGISTRY_USER`/`_PASSWORD`,
  or `~/.docker/config.json`; credentials never logged or persisted.
- **`EXPOSE` statement** parsed per the documented grammar; identity port
  publishing enforced (Linux host-network publish, VM multi-`hostfwd`).
  Host/container remapping fails closed with an actionable error.
- **Restart policies** (`never` / `on-failure` / `always`) and **healthchecks**
  enforced daemonlessly on every reconciliation pass (`ctst ps` / `ctst run`).
- Privileged Linux CI job running the root-only namespace/cgroup/mount fixtures.
- Packaging: in-tree Homebrew formula, `.deb`/`.rpm` via nfpm in the release
  workflow, winget manifest template, cosign-signed `SHA256SUMS`.

### Changed

- State schema bumped to 3 (`ports`, `restart`, `healthcheck`, `health`,
  `restart_count` on container entries); older states migrate automatically.
- Preset hints for uncurated images now point at `ctst pull`.

### Fixed

- `CgroupManager::destroy` used `remove_dir_all`, which cgroupfs rejects;
  now uses plain `rmdir`. Controllers (`cpu`, `memory`, `io`) are enabled in
  the parent cgroup's `subtree_control` on create.
- User-namespace test fixtures fork a single-threaded child so
  `unshare(CLONE_NEWUSER)` can succeed under the test harness.
- `examples/alpine_preset.ctst` used unsupported `#` comments.

## [1.0.5] — 2026-07-22

### Fixed

- Windows: container volume targets are Unix-absolute (`/app`), not `Path::is_absolute`.
- QEMU aarch64: use `virtio-net-pci` so the guest gets `eth0` and agent hostfwd works.
- VM agent: line-delimited RPC read + FIFO listen loop (BusyBox `nc` without `-e`).
- QEMU: pin hostfwd to `127.0.0.1` → guest `10.0.2.15`; capture guest serial on boot failure.

## [1.0.4] — 2026-07-22

### Fixed

- QEMU: use TCG under `CI=true` (GitHub macOS has no HVF).
- Volume specs: parse Windows drive-letter sources (`C:\data:/app`).

## [1.0.3] — 2026-07-22

### Fixed

- Windows clippy on `nix_kill` / unused `GRACEFUL_WAIT`.
- QEMU aarch64: `virt,gic-version=3`, `host` CPU, `virtio-net-device`, HVF→TCG fallback, stderr diagnostics.

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
