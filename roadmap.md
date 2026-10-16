# Containust Roadmap

This roadmap converts the current audit into an implementation sequence. It is intentionally delivery-oriented: every milestone has a bounded outcome, acceptance criteria, and the tests required to call it complete.

## Current baseline

Containust is at GA **`1.3.1`** (Sprint 12 complete: declarative plan/apply + performance pass; `1.3.1` adds `ctst download` and fixes the packaging bump). Sprint 11 (`1.2.0`) shipped: user/PID namespace spawn, remappable ports, named networks, DNS foundations, packaging automation, and opt-in OCI provenance. License is the proprietary [Containust Commercial License](LICENSE) (source-available; see [COMMERCIAL.md](COMMERCIAL.md)) — **`v1.2.0` is the first commercial-licensed tag**.

- CI green on Linux, macOS, Windows, QEMU smoke, privileged Linux (including `spawn_user_pid` + offline gate under default-on userns), format/clippy/deny/coverage.
- Release `v1.3.1` publishes tarballs/zips for five targets, `.deb`/`.rpm`, aggregated `SHA256SUMS`, and cosign keyless signature; packaging-bump refreshes Homebrew/winget.
- Images: `file://`, `tar://`, `preset://alpine|busybox`, `oci://` / `ctst pull` (Hub + GHCR, auth, digest-pin, `--offline` fail-closed, optional `--require-provenance`), catalog as `image://…@sha256:`.
- Runtime: user + PID namespaces default-on on Linux; `EXPOSE host:container` remap (userspace forwarder / VM `hostfwd`); named networks + `/etc/hosts` peers; restart/healthcheck; state schema **4**.
- Docs: product landing + HTML docs site under [`site/`](site/); operator guide [`docs/HowToUse.md`](docs/HowToUse.md).
- Plan/apply: `ctst plan` diffs (`+ ~ > = -`, `--json`, `--detailed-exitcode`), idempotent `ctst run` with dependency-ordered replacement and `--prune` (`1.3.0`).
- Deferred (Sprint 13+): zero-downtime rolling updates, volume drivers, remote orchestration, Apple notarization / Authenticode.

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
| Sprint 10 | `1.1.0` | Production-usable: OCI pull, ports/restart/healthcheck, privileged CI, packaging. |
| Sprint 11 | `1.2.0` | Isolation depth + networking: user/PID ns spawn, Linux port remap, DNS foundations. |
| Sprint 12 | `1.3.0` | Declarative plan/apply and hot-path performance. |
| Sprint 13 | `1.4.0` | Zero-downtime updates: start-before-stop with health-gated port handoff. |

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
- [x] **R1.2 Complete Linux `ContainerConfig` application.** Commands, environment, memory, CPU, read-only rootfs, and validated volumes are applied. Unsupported runtime properties fail closed. Identity port publish landed in Sprint 10; remapping is Sprint 11.
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

- [x] **B8.1 Feature freeze.** Policy documented in `docs/process/FEATURE_FREEZE.md` (enforced from `0.9.0-beta` tag).
- [x] **B8.2 Compatibility matrix.** `crates/containust-runtime/tests/compat_matrix.rs` covers state migration, `.ctst` parse/resolve, and error codes.
- [x] **B8.3 Upgrade rehearsal.** `upgrade_rehearsal` tests + runbook checklist: migrate, interrupted write, rollback preserving logs/catalog.
- [x] **B8.4 Release candidate.** Version `0.9.0-beta.1` + `docs/process/BETA_RC.md` clean-machine matrix; tag `v0.9.0-beta.1` to publish artifacts.

**Exit gate: passed for engineering scope.** Operator clean-machine dual-install evidence is recorded per `docs/process/BETA_RC.md` when cutting the GitHub Release.

## Sprint 9: `1.0.0` GA

**Goal:** ship only the behavior that is supported, documented, and operationally recoverable.

- [x] **G9.1 Final security sign-off.** Documented in `docs/process/GA_CHECKLIST.md` (threat model, deny/audit CI, fail-closed offline/digests, unsafe policy).
- [x] **G9.2 Final performance sign-off.** Parse/resolve + import regression budgets; `docs/PERFORMANCE.md`.
- [x] **G9.3 Support policy.** `docs/SUPPORT_POLICY.md` (platforms, SemVer, severity, deprecation).
- [x] **G9.4 GA release.** Workspace `1.0.0`; tag `v1.0.0` publishes checksummed artifacts (signing still deferred per `PACKAGING.md`).

**Exit gate: passed for GA engineering scope.** Known limitations remain listed as deferred in SUPPORT_POLICY / PACKAGING.

## Sprint 10: Production-usable v1 (`1.1.0`) — complete

**Goal:** make Containust usable in production without building from source: installable binaries, OCI pulls, and the runtime features the grammar already promises. Gated by `docs/process/PROD_CHECKLIST.md`.

### Wave 1 — Foundation

- [x] **P10.1 Tracker/doc sync.** `work.md`, `roadmap.md`, and `docs/process/PROD_CHECKLIST.md` reflect the current version and Sprint 10 scope.
- [x] **P10.2 Privileged Linux CI.** A `privileged-linux` CI job runs the `#[ignore]` core fixtures and the sprint3 offline gate as root on `ubuntu-latest` (cgroup v2, busybox-static).
- [x] **P10.3 Port publish docs.** Linux port publish behavior documented in `CLI_REFERENCE.md` / `SUPPORT_POLICY.md`.

### Wave 2 — OCI image pull

- [x] **P10.4 `oci://` scheme.** Registry resolution: manifest index → platform manifest → layer blobs (Docker Hub, GHCR).
- [x] **P10.5 Fail-closed policy.** Digest pin required by default; `--offline` rejects registry references before any connection.
- [x] **P10.6 Auth.** `CONTAINUST_REGISTRY_TOKEN` / `~/.docker/config.json`; secrets never logged or persisted.
- [x] **P10.7 Catalog import.** Pulled layers stored content-addressed and registered as `image://name@sha256:...`.
- [x] **P10.8 CLI + docs.** `ctst pull`, updated preset hints, CLI reference and examples.

### Wave 3 — Runtime enforcement of promised features

- [x] **P10.9 Ports.** Identity `ports` / `EXPOSE` published on Linux (host-net) and forwarded on the VM backend; singular `port` keeps CONNECT semantics.
- [x] **P10.10 Restart policies.** `never` / `on-failure` / `always` enforced via the state machine and reconciliation.
- [x] **P10.11 Healthchecks.** Interval execution, unhealthy marking, restart-policy integration.
- [x] **P10.12 State migration.** Schema bumped to 3; legacy states migrate via serde defaults.
- [x] **P10.13 Example gate.** Bundled examples parse/validate; healthcheck example deploys against the fake backend.

### Wave 4 — Packaging and release

- [x] **P10.14 Homebrew.** In-tree `Formula/ctst.rb` + install docs.
- [x] **P10.15 deb/RPM.** nfpm packaging in the release workflow.
- [x] **P10.16 winget.** Manifest template in `packaging/winget/`.
- [x] **P10.17 Signing/verification.** Cosign keyless on `SHA256SUMS`; verify procedure in `RUNBOOKS.md`.
- [x] **P10.18 Release.** CHANGELOG, SUPPORT_POLICY pruning, tag `v1.1.0` with green CI.

**Exit gate: passed.** Tag `v1.1.0` published with packages and signed checksums.

## Sprint 11: Isolation depth and networking (`1.2.0`)

**Goal:** close the largest remaining gaps vs Docker for real multi-service apps: proper Linux namespace spawn, remappable port publish, and basic service discovery — without remote orchestration.

### Wave 1 — Linux spawn isolation

- [x] **P11.1 User + PID namespaces on spawn.** Pipe-synced fork/exec path (`process_spawn.rs`) with uid/gid maps and post-`CLONE_NEWPID` double-fork; **default-on** for Linux deploy/spawn (`/dev/pts` best-effort under userns).
- [x] **P11.2 Privileged CI expand.** `spawn_user_pid` + offline gate run as root with default-on user/PID namespaces.
- [x] **P11.3 Docs.** `SUPPORT_POLICY.md` + `docs/HowToUse.md` cover userns/PID spawn; operator guide added.

### Wave 2 — Port remapping and networks

- [x] **P11.4 Linux port remapping.** `EXPOSE host:container` via userspace TCP forwarder into the container/shared netns (no `CAP_NET_ADMIN`); VM `hostfwd` remap on macOS/Windows; state schema 4.
- [x] **P11.5 Named networks.** `network = "bridge" | "host" | "none" | <name>` with persisted shared netns per project network.
- [x] **P11.6 DNS foundations.** Shared-netns peers written into container `/etc/hosts` as `127.0.0.1` for `CONNECT` `_HOST` resolution (no full mesh DNS yet).

### Wave 3 — Operator polish

- [x] **P11.7 Homebrew tap.** `scripts/bump_packaging.sh` + `release.yml` `packaging-bump` job; tap bootstrap docs in `packaging/homebrew-tap/README.md` (`brew tap RemiPelloux/containust` once the tap repo + `HOMEBREW_TAP_TOKEN` exist).
- [x] **P11.8 winget submission.** Same bump job refreshes `packaging/winget/Containust.ctst.yaml`; submit path documented in `packaging/winget/README.md` (`wingetcreate submit` after each release PR).
- [x] **P11.9 OCI provenance.** Opt-in `ctst pull --require-provenance` / `CONTAINUST_REQUIRE_PROVENANCE` runs `cosign verify` fail-closed before layer download.

**Exit gate: passed for engineering scope.** Tag `v1.2.0`. Optional follow-ups: create `RemiPelloux/homebrew-containust` + `HOMEBREW_TAP_TOKEN`; `wingetcreate submit` after packaging-bump PR lands with real sha256.

## Sprint 12: Declarative plan/apply and performance (`1.3.0`)

**Goal:** make `ctst run` safe to re-run and `ctst plan` an honest diff, and remove the biggest latency costs on everyday paths.

### Wave 1 — Plan/apply

- [x] **P12.1 Spec fingerprint + ledger.** SHA-256 fingerprint per component; `applied.json` ledger next to `state.json`, atomically written after every applied component.
- [x] **P12.2 `ctst plan` diff.** `create` / `replace` / `start` / `keep` / `remove` in dependency order with reasons; `--json`; `--detailed-exitcode`; read-only on fresh projects.
- [x] **P12.3 Idempotent `ctst run`.** Keep unchanged, start stopped, replace changed one at a time in topological order with dependent cascade; `--prune` for undeclared containers.
- [x] **P12.4 Tests.** Planner unit tests, stateful fake-backend apply tests (idempotency, cascade, prune), CLI `plan --json` / `--detailed-exitcode` tests.

### Wave 2 — Performance

- [x] **P12.5 Stop latency.** Poll-for-exit instead of fixed 2 s sleep; signal outside the state lock.
- [x] **P12.6 Create without lock contention.** Staged rootfs build outside the exclusive lock.
- [x] **P12.7 Pull layer reuse.** Skip download for layers already in the store.
- [x] **P12.8 I/O paths.** Buffered extract/pack, fewer stats, whiteout walk without following symlinks, clone-free state save.
- [x] **P12.9 Perf gates.** Stop latency + 10k-file extraction budgets.

### Wave 3 — Docs

- [x] **P12.10 README.** Animated hero/terminal/architecture SVGs (reduced-motion aware), use cases, comparison.

**Exit gate: passed for engineering scope.** Tag `v1.3.0`.

Patch `1.3.1`: `ctst download` (offline/on-prem prefetch), packaging-bump
CRLF fix with a non-fatal PR step, `ctst doctor` / `ctst download` docs.

## Sprint 13: Zero-downtime updates (`1.4.0`)

**Goal:** `ctst run` replaces a changed component without dropping traffic
on its published ports: start the new container, wait until it is healthy,
move the port to it, then retire the old one.

Today `create_or_replace` (`crates/containust-runtime/src/engine.rs`) retires
the old container *before* deploying the new one, and each port forwarder
(`port_forward.rs`) is bound to one container's netns for its whole life, so
every replace has a gap of stop time + start time + boot time.

### Design

- **Update strategy per component.** New `.ctst` property
  `update = "recreate" | "start-first"`. The default stays `recreate`, so
  existing files behave exactly as in `1.3.x`. `start-first` requires a
  `healthcheck` (validated at parse time with a clear error) because the swap
  is gated on it.
- **Stable forwarders with a swappable target.** A published host port is
  owned by a long-lived forwarder that reads its target netns from a small
  per-port target file (atomic rename) for every new connection. A swap is
  one rename: new connections reach the new container, in-flight connections
  finish on the old one.
- **Staged deploy.** The new container is created under a temporary name
  (`<name>~next`) with the same spec. When healthy, the target file is
  swapped, the old container is drained (configurable `stop_grace`, default
  2 s) and retired, and the new one is renamed to `<name>`; the ledger is
  written last, so an interrupted update is retried and its `~next` leftover
  removed on the next run.
- **Failure is non-destructive.** If the new container does not become
  healthy within `start_period + retries × interval`, it is removed and the
  old container keeps serving; `ctst run` exits non-zero with the probe
  output. `ctst plan` shows `~ replace (start-first)`.
- **Scope limits (documented, not silently degraded).** Identity publishes
  on the host network cannot bind twice, so they fall back to `recreate`
  with a plan warning. Stateful components with exclusive volumes should keep
  `recreate`; the docs say so. The VM backend gets the same flow through the
  agent; QEMU `hostfwd` stays fixed and the in-VM forwarder does the swap.

### Wave 1 — Runtime

- [x] **P13.1 Swappable forwarder target.** Per-port target file +
  per-connection lookup; forwarder survives container replacement; tests for
  swap under load (no refused connections) and stale-target cleanup.
- [x] **P13.2 Staged replace in the apply engine.** `start-first` path in
  `ApplyRun`: deploy `~next`, health gate, swap, drain, retire, rename;
  crash-safe ordering (ledger + state written so `ctst ps` reconciles a
  half-finished swap).
- [x] **P13.3 Health gate.** Reuse `supervise` probes synchronously with a
  bounded wait; probe output surfaced in the error.

### Wave 2 — Language, plan, VM

- [x] **P13.4 `.ctst` grammar.** `update` and `stop_grace` properties,
  validation (`start-first` needs `healthcheck`), fingerprint excludes
  `update` so changing strategy alone is not a replace.
- [x] **P13.5 Plan output.** Strategy in text and `--json`; fallback warnings.
- [ ] **P13.6 VM backend.** Agent-side forwarder swap on macOS/Windows; QEMU
  smoke covers one start-first replace. *Deferred to Sprint 14:* the VM
  backend reports no handoff support, so start-first falls back to
  `recreate` there with a plan note (no silent degradation).

### Wave 3 — Proof and docs

- [x] **P13.7 Downtime gate.** Integration test (privileged Linux CI): a
  client loops HTTP requests against a published port while `ctst run`
  replaces the component; zero failed requests for `start-first`.
- [x] **P13.8 Docs.** CTST_LANG, CLI_REFERENCE, HowToUse, site, SUPPORT_POLICY
  (remove blue/green from the deferred list), CHANGELOG.

**Exit gate:** P13.7 green on privileged Linux CI and QEMU smoke; `recreate`
behavior unchanged (existing apply tests untouched); tag `v1.4.0`.

### Also in Sprint 13 if capacity allows

- Parallel layer downloads for `ctst pull` / `ctst download` (bounded
  concurrency, same digest verification).
- Bump GitHub Actions off the deprecated Node 20 runtime
  (`actions/checkout`, `download-artifact`, `upload-artifact`).

## Later feature backlog

These are intentionally after Sprint 11:

- [x] Registry authentication and OCI image/index support (`ctst pull`, `1.1.0`).
- [x] Curated `preset://` catalog for Alpine/BusyBox (`ctst images --presets`).
- [x] Restart policies and healthcheck enforcement (`1.1.0`).
- [x] Identity port mappings + `EXPOSE` (`1.1.0`).
- [x] Declarative update/diff/apply semantics for `ctst plan` and `ctst run` (`1.3.0`).
- [x] Dependency-aware replacement of running components (`1.3.0`).
- [ ] Zero-downtime (start-before-stop) rolling updates — planned for Sprint 13 (`1.4.0`).
- [ ] Volume drivers, snapshots, backup/restore, and encrypted local storage.
- [ ] SDK async lifecycle API, typed events, backend injection.
- [ ] Remote execution or orchestration only after local security and lifecycle semantics are stable.
- [ ] Apple notarization / Windows Authenticode (cosign keyless already covers `SHA256SUMS`).
- [~] Performance: single-pass import hashing and preset cache reuse shipped; lazy layers, parallel extraction, and syscall overhead benchmarks remain.

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
