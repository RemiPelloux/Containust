# Containust Roadmap

This roadmap converts the current audit into an implementation sequence. It is intentionally delivery-oriented: every milestone has a bounded outcome, acceptance criteria, and the tests required to call it complete.

## Current baseline

Containust is at alpha `0.1.0`.

- The deterministic workspace suite passes 404 tests from 427 collected tests; 23 privileged tests are intentionally ignored.
- Formatting and strict Clippy pass locally when invoked with the installed toolchain binaries.
- The workspace compiles and its deterministic tests pass on Linux with the declared Rust 1.88 minimum toolchain.
- `cargo audit` and `cargo deny check` pass for the locked dependency graph.
- The parser, graph resolver, local image primitives, state/log persistence, CLI parsing, and Compose conversion are the most reliable parts of the product.
- Linux isolation, cgroups, mounts, QEMU, and eBPF remain platform-dependent and are not release-validated.
- A privileged Docker Linux run passes 20 of 25 privileged fixtures; five cgroup/user-namespace fixtures remain blocked by Docker Desktop host delegation and still require a supported Linux host.
- Sprint 1 wires `--offline`, `CONTAINUST_OFFLINE`, and `--state-file` through the CLI and engine; actual privileged-host validation and port forwarding remain deferred.

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

## Next sprint: Runtime correctness and usable CLI

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

- [ ] **P2.1 Project-scoped backend.** Pass the project data directory into Linux and VM backends instead of using a process-global data directory for all projects.
- [ ] **P2.2 Atomic state writes.** Write state to a temporary file, fsync where supported, then rename; recover cleanly from an interrupted write.
- [ ] **P2.3 State schema versioning.** Add a schema version and migration path for existing state files, including the command/env fields added in the current stabilization pass.
- [ ] **P2.4 Reconciliation.** On `ps`, detect dead PIDs, stale `Running` entries, orphaned rootfs directories, and abandoned cgroups.
- [ ] **P2.5 Cleanup guarantees.** Remove rootfs, volumes, logs, and cgroups according to explicit `stop` versus `remove` semantics.
- [ ] **P2.6 Concurrency control.** Add a project lock so two CLI processes cannot corrupt the same state index.

**Exit gate:** two independent project directories can run and clean up containers without sharing state, image entries, logs, or rootfs paths; crash/restart tests pass.

## Sprint 3: Image pipeline and offline operation

**Goal:** make local and remote image handling explicit, reproducible, and safe for air-gapped use.

- [ ] **I3.1 Source model.** Define image references with scheme, digest, transport, and local cache key instead of passing unstructured strings through the runtime.
- [ ] **I3.2 Local import.** Implement deterministic import of directory and tar/tar.gz sources into content-addressed layers.
- [ ] **I3.3 Remote fetch.** Add an explicit opt-in downloader with timeouts, redirect policy, size limits, retries, and SHA-256 verification.
- [ ] **I3.4 Offline enforcement.** Reject HTTP(S), remote imports, and uncached layers before opening a network connection when offline mode is enabled.
- [ ] **I3.5 Catalog integrity.** Deduplicate registrations, validate layer references, and add catalog locking/atomic writes.
- [ ] **I3.6 Build behavior.** Change `ctst build` from source inspection to a real build/import operation, with a dry-run mode for planning only.
- [ ] **I3.7 Supply-chain metadata.** Store source URL, digest, creation time, and tool version for each image entry.

**Exit gate:** an image can be imported online once, verified by digest, exported or copied into an air-gapped environment, and run with `--offline` without network access.

## Sprint 4: Security hardening

**Goal:** validate and enforce the security model rather than only exposing security-shaped options.

- [ ] **S4.1 Rootfs safety.** Reject path traversal and unsafe tar entries; do not follow symlinks outside the extraction root.
- [ ] **S4.2 Capability policy.** Define a minimal default capability set, validate allowlists, and test the effective capability drop in a privileged Linux fixture.
- [ ] **S4.3 Namespace policy.** Make user, PID, mount, UTS, IPC, and network namespace choices explicit in the runtime config and validate unsupported combinations.
- [ ] **S4.4 Mount and volume policy.** Validate source/target paths, prevent host escape, enforce read-only mounts, and cleanly unmount on teardown.
- [ ] **S4.5 Resource limits.** Validate CPU/memory ranges and fail closed when an explicitly requested cgroup limit cannot be applied.
- [ ] **S4.6 Secret handling.** Keep secrets out of state, logs, debug output, and generated plans; add redaction tests.
- [ ] **S4.7 Threat-model review.** Update the repository threat model and run `cargo deny`, advisory, license, and unsafe-code reviews in CI.

**Exit gate:** security-sensitive inputs have negative tests, privileged tests pass on the supported Linux matrix, and an independent review signs off on the threat model.

## Sprint 5: Cross-platform VM backend

**Goal:** make macOS and Windows execution a supported workflow with deterministic assets and lifecycle behavior.

- [ ] **V5.1 VM asset manifest.** Pin kernel/initramfs URLs and SHA-256 digests per architecture.
- [ ] **V5.2 Asset cache.** Add resumable downloads, verification, cache locking, and clear offline failure messages.
- [ ] **V5.3 VM lifecycle.** Implement idempotent start/stop, readiness checks, graceful shutdown, and stale-process recovery.
- [ ] **V5.4 RPC contract.** Version the agent protocol, validate all responses, add request IDs, timeouts, and bounded payload sizes.
- [ ] **V5.5 Port forwarding.** Track ownership and teardown of forwarded ports; reject collisions before boot.
- [ ] **V5.6 Cross-platform CI.** Run compile/unit tests on macOS and Windows and execute a QEMU smoke test on at least one hosted platform.

**Exit gate:** a documented macOS and Windows quickstart can build, boot, run, inspect, and stop a local image without manual VM cleanup.

## Sprint 6: Observability and operator experience

**Goal:** make failures diagnosable during repeated operational use.

- [ ] **O6.1 Structured events.** Emit lifecycle events with container ID, project, operation, duration, and error code.
- [ ] **O6.2 Metrics correctness.** Validate CPU, memory, I/O, and process metrics against a known workload; define zero/unavailable semantics.
- [ ] **O6.3 TUI integration.** Wire `ctst ps --tui` to the real engine and support refresh, selection, logs, and quit behavior.
- [ ] **O6.4 eBPF lifecycle.** Implement feature-gated load/attach/detach, capability checks, and graceful degradation when unsupported.
- [ ] **O6.5 Diagnostics.** Add `ctst doctor` for platform, cgroups, namespace, mount, QEMU, cache, and permissions checks.
- [ ] **O6.6 Error UX.** Standardize exit codes and include a remediation hint for every user-facing runtime error.

**Exit gate:** a failure can be diagnosed from CLI output and logs without reading source code; TUI and eBPF are clearly reported as unavailable when prerequisites are missing.

## Sprint 7: Release readiness

**Goal:** establish a repeatable release process with evidence for supported platforms.

- [ ] **L7.1 Versioning.** Centralize workspace versioning and document compatibility guarantees for `.ctst`, state files, and the SDK.
- [ ] **L7.2 Release artifacts.** Produce signed/checksummed binaries for Linux, macOS, and Windows with reproducible build metadata.
- [ ] **L7.3 Packaging.** Add Homebrew, Debian, RPM, and Windows installation paths or explicitly defer each with an issue and owner.
- [ ] **L7.4 CI gates.** Require check, format, clippy, deterministic tests, privileged Linux tests, dependency audit, and documentation checks before release.
- [ ] **L7.5 Coverage.** Publish library coverage and track regressions; target 90% for stable library crates.
- [ ] **L7.6 Performance.** Benchmark parse, graph resolution, image import, startup, and teardown; set regression budgets.
- [ ] **L7.7 Runbooks.** Add upgrade, rollback, incident, cache recovery, and data cleanup procedures.

**Exit gate:** a tagged release can be installed from a clean machine, verified, and rolled back using only published documentation.

## Sprint 8: Beta stabilization

**Goal:** freeze the feature surface and prove upgrade compatibility before `1.0.0`.

- [ ] **B8.1 Feature freeze.** No new runtime features after the beta tag; only correctness, security, performance, and documentation fixes.
- [ ] **B8.2 Compatibility matrix.** Test state/schema migration, `.ctst` parsing, CLI exit codes, and SDK behavior across the previous two minor releases.
- [ ] **B8.3 Upgrade rehearsal.** Upgrade a running project, recover interrupted state writes, and roll back without losing image metadata or logs.
- [ ] **B8.4 Release candidate.** Publish `0.9.0-beta` artifacts and require two independent clean-machine installation runs per supported platform.

**Exit gate:** no open P0/P1 correctness or security issues, migration rehearsal succeeds, and release artifacts are reproducible.

## Sprint 9: `1.0.0` GA

**Goal:** ship only the behavior that is supported, documented, and operationally recoverable.

- [ ] **G9.1 Final security sign-off.** Threat model, dependency audit, unsafe-code review, rootfs extraction review, and privileged Linux tests are complete.
- [ ] **G9.2 Final performance sign-off.** Parse/plan, image import, startup, stop, state writes, and log follow meet published budgets on the reference machines.
- [ ] **G9.3 Support policy.** Publish supported OS/kernel/QEMU versions, compatibility guarantees, issue severity definitions, and deprecation policy.
- [ ] **G9.4 GA release.** Tag `1.0.0`, publish signed artifacts/checksums, update documentation, and archive the release checklist.

**Exit gate:** all release checks are green, known limitations are documented as supported/deferred behavior, and rollback/runbooks have been exercised.

## Later feature backlog

These are intentionally after correctness and release gates:

- [ ] Registry authentication, OCI image/index support, and signed image metadata.
- [ ] Multi-network networking, DNS/service discovery, and explicit port mappings.
- [ ] Restart policies and healthcheck enforcement in the runtime state machine.
- [ ] Declarative update/diff/apply semantics for `ctst plan` and `ctst run`.
- [ ] Rolling updates and dependency-aware replacement of running components.
- [ ] Volume drivers, snapshots, backup/restore, and encrypted local storage.
- [ ] SDK async lifecycle API, typed events, backend injection, and API stability policy.
- [ ] Remote execution or orchestration only after local security and lifecycle semantics are stable.
- [ ] Performance work: lazy layers, parallel extraction, startup caching, and syscall overhead benchmarks.

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
