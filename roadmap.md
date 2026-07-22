# Containust Roadmap

This roadmap converts the current audit into an implementation sequence. It is intentionally delivery-oriented: every milestone has a bounded outcome, acceptance criteria, and the tests required to call it complete.

## Current baseline

Containust is at GA `1.0.2` after Sprint 9 security/perf/support sign-off and the Windows/QEMU CI patch release.

- The deterministic macOS workspace suite passes 470 tests with 23 privileged tests intentionally ignored. The Rust 1.88 Linux suite passes 480 with 26 privileged tests ignored.
- Formatting and strict Clippy pass locally when invoked with the installed toolchain binaries.
- The workspace compiles and its deterministic tests pass on Linux with the declared Rust 1.88 minimum toolchain.
- `cargo audit` and `cargo deny check` pass for the locked dependency graph.
- The parser, graph resolver, local image primitives, state/log persistence, CLI parsing, and Compose conversion are the most reliable parts of the product.
- Linux isolation, cgroups, mounts, QEMU, and eBPF remain platform-dependent and are not release-validated.
- A privileged Docker Linux run passes 20 of 25 privileged fixtures; five cgroup/user-namespace fixtures remain blocked by Docker Desktop host delegation and still require a supported Linux host.
- Sprint 1 wires `--offline`, `CONTAINUST_OFFLINE`, and `--state-file` through the CLI and engine; actual privileged-host validation and port forwarding remain deferred.
- Sprint 2 adds project-scoped storage, atomic schema-versioned state, cross-process locking, lifecycle reconciliation, and explicit `stop`/`rm` cleanup semantics.
- Sprint 3 adds structured image references, deterministic content-addressed import, an opt-in digest-verified remote fetcher, a locked/atomic image catalog with supply-chain metadata, a real `ctst build` with `--dry-run`, and offline-safe `image://` execution. The full exit gate (online import, air-gapped copy, `--offline` run) passes as a privileged Linux fixture.
- Post–Sprint 3: curated `preset://alpine` / `preset://busybox` downloads (pinned Alpine minirootfs) with `ctst images --presets`; Docker Hub names like `node`/`php` return actionable hints until OCI pull lands.
- Post–Sprint 3 performance pass (`0.4.2`): the import pipeline hashes in a single pass (directory packing, remote downloads, and staged archives are digested while written instead of re-read), cached presets are reused in place without a staging copy so repeated `preset://` deploys cost one verification read, and `crates/containust-image/tests/perf_regression.rs` gates 32 MiB imports and staging-file hygiene. Measured on a 256 MiB image: cold `tar://` import 0.87 s → 0.50 s, warm re-import 0.61 s → 0.50 s, `file://` import 0.72 s → 0.65 s.

## Release train

| Sprint | Target release | Gate |
| --- | --- | --- |
| Sprint 1 | `0.2.0` | Runtime correctness, configuration propagation, and deterministic CLI lifecycle behavior. |
| Sprint 2 | `0.3.0` | Project isolation, atomic state, reconciliation, and concurrency safety. |
| Sprint 3 | `0.4.0` | Content-addressed image import and enforced offline operation. |
| Sprint 4 | `0.5.0` | Security hardening and privileged Linux validation. |
| Sprint 5 | `0.6.0` | Supported QEMU lifecycle on macOS and Windows. |
| Sprint 6 | `0.7.0` | Diagnostics, metrics, TUI, and observability lifecycle. |
| Sprint 7 | `0.8.0` | Release packaging, coverage, benchmarks, and operational runbooks. |
| Sprint 8 | `0.9.0-beta` | Feature freeze, compatibility review, upgrade rehearsal, and release candidate. |
| Sprint 9 | `1.0.0` | GA only after every platform/security/recovery gate is green. |

## Delivery policy

Each sprint is two weeks unless the team changes the capacity assumption. Work is ordered by user-visible correctness and security, not by subsystem ownership.

Every feature must include:

1. A design note or command/API contract before implementation.
2. Unit tests for pure logic and failure paths.
3. An integration test where the feature crosses crate boundaries.
4. Updated CLI/docs/examples.
5. A verification report covering `cargo check`, `cargo fmt`, `cargo clippy`, tests, and dependency audit status.

Release work cannot be marked complete when a feature is only parser-supported. The runtime, persistence, error behavior, and CLI contract must all agree.

## Sprint 1: Runtime correctness and usable CLI

**Goal:** make one local-rootfs composition reliable from parse through start, inspect, exec, logs, and stop on a supported Linux host.

### Sprint backlog

- [x] **R1.1 Wire global configuration.** Thread `--offline`, `--state-file`, and `CONTAINUST_OFFLINE` through command dispatch and engine construction. Remove silent fallbacks when a requested state file cannot be opened.
- [~] **R1.2 Complete Linux `ContainerConfig` application.** Commands, environment, memory, CPU, read-only rootfs, and validated volumes are applied. Unsupported runtime properties fail closed. Port forwarding remains a Sprint 5 networking item.
- [x] **R1.3 Fix lifecycle semantics.** Implement real force-stop behavior, make repeated stop/remove operations deterministic, and record `Failed` state when start fails after creation.
- [x] **R1.4 Make command targeting consistent.** Resolve container names and IDs in `stop`, `logs`, and `exec` through one shared API.
- [x] **R1.5 Make log following real.** Implement `ctst logs --follow` with Ctrl+C cancellation and incremental byte-offset reads.
- [x] **R1.6 Fix Compose conversion contracts.** Add `entrypoint` to the `.ctst` grammar/runtime and validate generated output by parsing it before returning it.
- [x] **R1.7 Add runtime-focused tests.** Add fake-backend engine tests, offline failure tests, state/config tests, volume parser tests, and a large-graph performance regression test. Privileged host validation remains a release gate.

### Sprint acceptance criteria

- [x] A fake-backend composition test proves declared command, environment, resource limits, volumes, and lifecycle calls are passed correctly.
- [x] `--offline` rejects remote image/import access before any backend or network operation.
- [x] `--state-file /path/state.json` selects the state/log/image data directory for every relevant command.
- [x] Unsupported runtime properties fail with a specific error message.
- [x] No new warnings under `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] Privileged Linux fixture validates actual namespace, mount, volume, cgroup, and process behavior.

## Sprint 2: Project isolation and lifecycle durability

**Goal:** make storage behavior match the documented project model and survive crashes/restarts.

- [x] **P2.1 Project-scoped backend.** Linux and VM backends derive stable project identities and isolate state, logs, rootfs, and cgroups under the selected composition's `.containust/` directory.
- [x] **P2.2 Atomic state writes.** State writes use same-directory temporary files, file synchronization, atomic rename, and parent-directory synchronization where supported; interrupted temporary writes are ignored.
- [x] **P2.3 State schema versioning.** State schema version 2 migrates legacy unversioned/version-1 files and rejects unsupported future schemas.
- [x] **P2.4 Reconciliation.** `ctst ps` detects dead PIDs, marks stale `Running` entries failed, and removes orphaned project rootfs directories and cgroups.
- [x] **P2.5 Cleanup guarantees.** `stop` retains rootfs/logs for inspection and removes the cgroup; `ctst rm` removes project-owned rootfs, logs, cgroups, and state. Host volume sources remain untouched.
- [x] **P2.6 Concurrency control.** Shared/exclusive filesystem locks and transactional updates prevent competing CLI processes from corrupting a project state index.

**Exit gate: complete for deterministic coverage.** Two independent project fixtures create and clean up without sharing state, logs, rootfs paths, or cgroups. Legacy migration, interrupted writes, thread contention, and real subprocess contention are covered. Privileged native-Linux behavior remains part of the Sprint 4 host-validation gate.

## Sprint 3: Image pipeline and offline operation

**Goal:** make local and remote image handling explicit, reproducible, and safe for air-gapped use.

- [x] **I3.1 Source model.** `ImageReference` carries scheme (`file`, `tar`, `image`, `https`, `http`), location, optional pinned `@sha256:` digest, and a deterministic cache key. Parsing is pure (no I/O).
- [x] **I3.2 Local import.** Directories are packed into a canonical tar (sorted entries, zeroed timestamps, normalized ownership) and archives are copied verbatim; both are stored content-addressed under `layers/<sha256>/`. Importing the same source twice yields the same digest and reuses the layer.
- [x] **I3.3 Remote fetch.** `FetchPolicy` enforces a total timeout, a bounded redirect policy, a streaming size cap, and bounded retries. Remote references must pin a digest; downloads that fail verification are deleted.
- [x] **I3.4 Offline enforcement.** Offline mode rejects remote references in the compose validator, the import pipeline, and the fetcher itself — all before any connection is opened. Catalog (`image://`) references remain valid offline.
- [x] **I3.5 Catalog integrity.** Registrations are deduplicated by name, every referenced layer must exist in the store, and the catalog JSON is guarded by a shared/exclusive file lock with atomic temp-file/rename writes.
- [x] **I3.6 Build behavior.** `ctst build` now performs the real import into the project catalog and prints the resulting `image://name@sha256:` reference; `--dry-run` plans without writing.
- [x] **I3.7 Supply-chain metadata.** Each catalog entry records the source URI (with digest suffix), content digest, ISO-8601 creation time, and importing tool version. Legacy catalogs without these fields still load.

**Exit gate: passed.** An image imported online once is digest-verified, and after copying only `.containust/images/` and `.containust/layers/` into an air-gapped project (original source deleted), `ctst --offline build`/`plan` succeed and a privileged Linux fixture runs the container from `image://` with `--offline` and no network access.

### Sprint 3 definition of done (evidence)

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo deny check` pass.
- Deterministic suites: 470 passed / 0 failed on macOS, 480 passed / 0 failed on Linux (Rust 1.88, Docker).
- Exit-gate integration tests live in `crates/containust-cli/tests/sprint3_gate.rs`; the privileged `gate_offline_run_starts_container_from_catalog` fixture passed on a privileged Linux (aarch64) container with busybox-static, exercising import → digest verify → air-gap copy → `ctst --offline run` → `ps` shows `running` → forced stop.
- CLI reference (`docs/CLI_REFERENCE.md`) and `.ctst` language reference (`docs/CTST_LANG.md`) document the new build pipeline and the `image://` scheme.
- Backward compatibility: schema-less legacy catalog entries deserialize with defaulted metadata fields.

## Sprint 4: Security hardening — complete (`0.5.0`)

**Goal:** validate and enforce the security model rather than only exposing security-shaped options.

- [x] **S4.1 Rootfs safety.** Shared `safe_extract_archive` rejects absolute paths, `..`, hard links, device nodes, and escaping symlinks; wired into import, layer extract, and Linux `tar://` prep. Symlink-safe `copy_dir_recursive`.
- [x] **S4.2 Capability policy.** Drop-all default kept; `PR_SET_NO_NEW_PRIVS` applied; drop errors fail closed in the spawn path (no longer ignored). `CAP_SYS_ADMIN` remains absent from the allowlist enum.
- [x] **S4.3 Namespace policy.** `NamespaceConfig` is part of `ContainerConfig` / `ProcessConfig`. Defaults enable mount/network/IPC/UTS; unsupported PID/user requests fail closed via `validate_for_spawn`. Spawn uses `create_namespaces`.
- [x] **S4.4 Mount and volume policy.** Parent-process validation (`volume.rs`): absolute paths, no `..`, canonicalize existing sources, `ro`/`rw` only. Applied before create and again before spawn.
- [x] **S4.5 Resource limits.** Memory must be > 0; CPU shares in `1..=10000`. Explicit limits apply fail-closed through cgroups v2; on failure the just-spawned process is killed and the container is marked `Failed`.
- [x] **S4.6 Secret handling.** `containust_common::redact` redacts secret-looking env values in `state.json` and restores them at start from `CONTAINUST_SECRET_*` / host env (fail closed if missing).
- [x] **S4.7 Threat-model review.** Added `docs/THREAT_MODEL.md`. `cargo deny` + `cargo audit` remain in `.github/workflows/security.yml`.

**Exit gate: passed for deterministic coverage.** Negative extract/volume/redaction/namespace tests are green (503 passed / 0 failed / 23 ignored on macOS). Privileged effective-cap and cgroup enforcement fixtures remain host-gated (`#[ignore]`) for the supported Linux matrix. PID/user namespace wiring is deliberately deferred and fails closed when requested.

## Sprint 5: Cross-platform VM backend — complete (`0.6.0`)

**Goal:** make macOS and Windows execution a supported workflow with deterministic assets and lifecycle behavior.

- [x] **V5.1 VM asset manifest.** Pin kernel/initramfs URLs and SHA-256 digests per architecture (`backend/vm/assets.rs`, Alpine 3.21.7).
- [x] **V5.2 Asset cache.** Resumable downloads (`*.partial` + HTTP Range), digest verification, exclusive cache lock, offline fail-closed remediation.
- [x] **V5.3 VM lifecycle.** Pidfile-backed idempotent start/stop, agent readiness adopt, SIGTERM→SIGKILL (honor `--force`), stale pid recovery; shared VM survives CLI drop.
- [x] **V5.4 RPC contract.** Versioned line-delimited JSON (`v=1`, request IDs), I/O timeouts, request/response size caps, fail-closed parsing.
- [x] **V5.5 Port forwarding.** Probe bind collisions before boot; persist `forwarded_ports` in pidfile; reject hot-add when VM already running; clear ownership on stop.
- [x] **V5.6 Cross-platform CI.** macOS/Windows compile+test jobs; QEMU smoke on `macos-latest` (`vm start`/`stop` idempotent).

**Exit gate: passed for Sprint 5 scope.** Assets, cache, lifecycle, RPC, port ownership, and CI smoke are in place. Full guest container runbooks remain operator-validated on hardware with QEMU.

## Sprint 6: Observability and operator experience (`0.7.0`)

**Goal:** make failures diagnosable during repeated operational use.

- [x] **O6.1 Structured events.** Runtime `EventBus` + `Operation` events (project, operation, duration_ms, error_code); SDK `EventListener::subscribe`.
- [x] **O6.2 Metrics correctness.** `MetricAvailability` for CPU/memory/I/O; Linux cgroup/`io.stat` reads; non-Linux zeros mean unavailable, not idle.
- [x] **O6.3 TUI integration.** `ctst ps --tui` drives `containust_tui::run_dashboard` with live container rows, selection, and quit.
- [x] **O6.4 eBPF lifecycle.** Feature/OS-gated `attach`/`detach`, doctor status via `runtime::observe`, graceful degradation when unsupported.
- [x] **O6.5 Diagnostics.** `ctst doctor` for OS/arch, native/QEMU backend, cache writability, VM assets, offline, cgroup v2 (Linux).
- [x] **O6.6 Error UX.** Stable codes via `containust_common::codes` with CLI `error[CODE]` + remediation hint + exit status.

**Exit gate: passed for Sprint 6 scope.**

## Sprint 7: Release readiness (`0.8.0`)

**Goal:** establish a repeatable release process with evidence for supported platforms.

- [x] **L7.1 Versioning.** Workspace SemVer + `docs/VERSIONING.md` / `CHANGELOG.md`; `STATE_SCHEMA_VERSION` in common; SDK/CLI doc banners aligned.
- [x] **L7.2 Release artifacts.** Multi-target archives with SHA-256, `build-info.json`, and embedded `git=`/`built=` metadata (signing deferred — see PACKAGING.md).
- [x] **L7.3 Packaging.** Documented supported paths + explicit Homebrew/Debian/RPM/Windows deferrals with owners (`docs/PACKAGING.md`).
- [x] **L7.4 CI gates.** Existing check/fmt/clippy/test/deny + docs job (`cargo doc` + required markdown). Privileged Linux suite remains `#[ignore]` with tracking for GA.
- [x] **L7.5 Coverage.** `cargo llvm-cov` job uploads `lcov.info` artifact each CI run.
- [x] **L7.6 Performance.** Documented budgets; import + parse/resolve regression tests.
- [x] **L7.7 Runbooks.** Upgrade, rollback, incident, cache recovery, cleanup (`docs/RUNBOOKS.md`).

**Exit gate: passed for Sprint 7 scope.** Workspace is `0.8.0`; cut `v0.8.0` GitHub Release when ready to publish binaries.

## Sprint 8: Beta stabilization (`0.9.0-beta.1`)

**Goal:** freeze the feature surface and prove upgrade compatibility before `1.0.0`.

- [x] **B8.1 Feature freeze.** Policy documented in `docs/FEATURE_FREEZE.md` (enforced from `0.9.0-beta` tag).
- [x] **B8.2 Compatibility matrix.** `crates/containust-runtime/tests/compat_matrix.rs` covers state migration, `.ctst` parse/resolve, and error codes.
- [x] **B8.3 Upgrade rehearsal.** `upgrade_rehearsal` tests + runbook checklist: migrate, interrupted write, rollback preserving logs/catalog.
- [x] **B8.4 Release candidate.** Version `0.9.0-beta.1` + `docs/BETA_RC.md` clean-machine matrix; tag `v0.9.0-beta.1` to publish artifacts.

**Exit gate: passed for engineering scope.** Operator clean-machine dual-install evidence is recorded per `docs/BETA_RC.md` when cutting the GitHub Release.

## Sprint 9: `1.0.0` GA

**Goal:** ship only the behavior that is supported, documented, and operationally recoverable.

- [x] **G9.1 Final security sign-off.** Documented in `docs/GA_CHECKLIST.md` (threat model, deny/audit CI, fail-closed offline/digests, unsafe policy).
- [x] **G9.2 Final performance sign-off.** Parse/resolve + import regression budgets; `docs/PERFORMANCE.md`.
- [x] **G9.3 Support policy.** `docs/SUPPORT_POLICY.md` (platforms, SemVer, severity, deprecation).
- [x] **G9.4 GA release.** Workspace `1.0.0`; tag `v1.0.0` publishes checksummed artifacts (signing still deferred per `PACKAGING.md`).

**Exit gate: passed for GA engineering scope.** Known limitations remain listed as deferred in SUPPORT_POLICY / PACKAGING.

## Later feature backlog

These are intentionally after correctness and release gates:

- [ ] Registry authentication, OCI image/index support, and signed image metadata (enables `preset://node`, `preset://php`, arbitrary Hub names).
- [x] Curated `preset://` catalog for Alpine/BusyBox minirootfs with pinned digests and offline cache reuse (`ctst images --presets`).
- [ ] Multi-network networking, DNS/service discovery, and explicit port mappings.
- [ ] Restart policies and healthcheck enforcement in the runtime state machine.
- [ ] Declarative update/diff/apply semantics for `ctst plan` and `ctst run`.
- [ ] Rolling updates and dependency-aware replacement of running components.
- [ ] Volume drivers, snapshots, backup/restore, and encrypted local storage.
- [ ] SDK async lifecycle API, typed events, backend injection, and API stability policy.
- [ ] Remote execution or orchestration only after local security and lifecycle semantics are stable.
- [~] Performance work: single-pass import hashing, in-place preset cache reuse, and import perf regression gates shipped in `0.4.2`; lazy layers, parallel extraction, startup caching, and syscall overhead benchmarks remain for Sprint 7.

## Cross-cutting definition of done

A roadmap item is complete only when:

- behavior is implemented in the runtime, not only parsed or displayed;
- success, invalid input, unavailable platform, and cleanup paths are tested;
- state/log/schema changes are backward-compatible or migrated;
- CLI, SDK, examples, and reference docs agree;
- security implications and resource ownership are reviewed;
- CI verification is green, or the remaining environment prerequisite is documented with an owner and follow-up issue.

## Ownership and tracking

Use the IDs in this document (`R1.1`, `P2.1`, and so on) in commits, pull requests, and issues. Each sprint should select a small set of IDs, name an owner, record dependencies, and close with the relevant acceptance evidence.
