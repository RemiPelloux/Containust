# Containust — Product Readiness Work Tracker

> **Goal**: Ship Containust as a production-ready container runtime
> **Date**: 2026-04-02
> **Standards**: 90%+ test coverage for library crates, zero clippy warnings, no banned patterns

## Strict Coding Rules (ALL sub-agents must follow)
- **Functions**: max 25 lines
- **Files**: max 300 lines
- **Module public items**: max 10
- **Function params**: max 4 (else use struct)
- **Banned**: `.unwrap()` in lib crates, `panic!`, `todo!`, `dbg!`, `print!`, `println!`
- **Naming**: `snake_case` modules/fn, `PascalCase` types, `SCREAMING_SNAKE_CASE` constants
- **Error handling**: `Result<T, E>`, `thiserror`, no `.unwrap()` — use `?` with `.map_err()`
- **unsafe**: requires `// SAFETY:` comment with justification
- **Immutability**: create new objects, never mutate in-place
- **CI**: `cargo check`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo deny check`
- **Test naming**: `<unit>_<scenario>_<expected_outcome>()`

---

## Current Test Status (Baseline)

| Crate | Tests | Source Files | Needs Tests |
|---|---|---|---|
| containust-common | 27 | 5 | MINOR — constants.rs missing tests |
| containust-core | 4 | 14 | MAJOR — namespaces, cgroups, filesystem, capability |
| containust-compose | 66 | 8 | MODERATE — parser/mod.rs, resolver, distroless, import |
| containust-image | 31 | 6 | MODERATE — registry, source, fuse missing |
| containust-runtime | 45 | 10 | MAJOR — backend/mod.rs, engine, container edge cases |
| containust-ebpf | 22 | 5 | MODERATE — tracer, file/net monitor, programs |
| containust-sdk | 5 | 3 | MODERATE — builder, graph_resolver, event listener |
| containust-tui | 3 | 6 | MODERATE — app, dashboard, container views |
| containust-cli | 0 | 10 | CRITICAL — all commands untested |

---

## TIER 1 — Critical (Show-stoppers)

### 1. CI/CD Pipeline
- [ ] `1.1` Create `.github/workflows/ci.yml`
  - Jobs: check, fmt, clippy, test, deny
  - Matrix: Linux (full), macOS (check+test), Windows (check+test)
  - Trigger: push/PR to main
- [ ] `1.2` Create `deny.toml` (cargo-deny config)
- [ ] `1.3` Add `cargo-deny` action to CI
- [ ] `1.4` Verify CI config validity locally

### 2. containust-cli Tests (CRITICAL — 0 tests, 10 commands)
- [ ] `2.1` Test `commands/run.rs` — ctst run command parsing
- [ ] `2.2` Test `commands/ps.rs` — list containers
- [ ] `2.3` Test `commands/stop.rs` — stop container
- [ ] `2.4` Test `commands/exec.rs` — exec into container
- [ ] `2.5` Test `commands/logs.rs` — container log retrieval
- [ ] `2.6` Test `commands/images.rs` — image management
- [ ] `2.7` Test `commands/build.rs` — build command
- [ ] `2.8` Test `commands/plan.rs` — plan command
- [ ] `2.9` Test `commands/convert.rs` — docker-compose converter
- [ ] `2.10` Test CLI integration (subcommand routing via clap)

### 3. containust-core Tests (MAJOR — 14 source files)
- [ ] `3.1` Test `lib.rs` — module exports, public API
- [ ] `3.2` Test `namespace/mod.rs` — namespace flag combinations
- [ ] `3.3` Test `namespace/pid.rs` — PID namespace creation
- [ ] `3.4` Test `namespace/mount.rs` — mount namespace
- [ ] `3.5` Test `namespace/network.rs` — network namespace
- [ ] `3.6` Test `namespace/uts.rs` — UTS namespace (hostname)
- [ ] `3.7` Test `namespace/ipc.rs` — IPC namespace
- [ ] `3.8` Test `namespace/user.rs` — user namespace
- [ ] `3.9` Test `cgroup/mod.rs` — cgroup v2 manager
- [ ] `3.10` Test `cgroup/cpu.rs` — CPU resource limits
- [ ] `3.11` Test `cgroup/memory.rs` — memory limits
- [ ] `3.12` Test `cgroup/io.rs` — I/O limits
- [ ] `3.13` Test `filesystem/overlayfs.rs` — overlay mount
- [ ] `3.14` Test `filesystem/pivot_root.rs` — pivot_root
- [ ] `3.15` Test `filesystem/mount.rs` — bind mounts
- [ ] `3.16` Test `capability.rs` — capability dropping

### 4. containust-runtime Tests (MAJOR — engine crate)
- [ ] `4.1` Test `backend/mod.rs` — backend detection
- [ ] `4.2` Test `backend/linux.rs` — Linux native backend
- [ ] `4.3` Test `backend/vm/mod.rs` — VM backend
- [ ] `4.4` Test `backend/vm/initramfs.rs` — initramfs builder
- [ ] `4.5` Test `container.rs` — state machine transitions
- [ ] `4.6` Test `engine.rs` — deployment orchestration

### 5. Integration/E2E Tests
- [ ] `5.1` Fix `containust-runtime/tests/e2e_test.rs`
- [ ] `5.2` Create `tests/integration/` directory structure
- [ ] `5.3` Integration test: parse + validate `.ctst` file
- [ ] `5.4` Integration test: full deploy order resolution
- [ ] `5.5` Integration test: SDK lifecycle (create → start → stop)

---

## TIER 2 — Important (Before first release)

### 6. CLI — vm commands
- [ ] `6.1` `ctst vm start` — manual QEMU VM start
- [ ] `6.2` `ctst vm stop` — stop QEMU VM
- [ ] `6.3` Add to CLI command registration in `main.rs`

### 7. eBPF Feature Testing
- [ ] `7.1` Tests for `--features ebpf` in containust-ebpf
- [ ] `7.2` Test program loading/unloading lifecycle
- [ ] `7.3` Test tracer start/stop

### 8. containust-sdk Tests
- [ ] `8.1` Test `builder.rs` — ContainerBuilder fluent API
- [ ] `8.2` Test `graph_resolver.rs` — GraphResolver
- [ ] `8.3` Test `event.rs` — EventListener

### 9. containust-tui Tests
- [ ] `9.1` Test `app.rs` — app state machine
- [ ] `9.2` Test `ui/dashboard.rs` — dashboard rendering
- [ ] `9.3` Test `ui/container.rs` — container detail view

### 10. Additional Test Coverage (reach 90%)
- [ ] `10.1` containust-common `constants.rs` tests
- [ ] `10.2` containust-compose `distroless.rs` tests
- [ ] `10.3` containust-compose `import.rs` tests
- [ ] `10.4` containust-image `registry.rs` tests
- [ ] `10.5` containust-image `fuse.rs` tests
- [ ] `10.6` containust-runtime `container.rs` edge cases

---

## TIER 3 — Polish (Post-launch)
- [ ] `11` Distribution packaging (deb, rpm, Homebrew)
- [ ] `12` Publish to crates.io
- [ ] `13` Code coverage reporting with badges
- [ ] `14` Benchmark suite
- [ ] `15` Example templates
- [ ] `16` API stability audit

---

## Parallel Execution Plan

| Track | Sub-agent | Scope | Tasks |
|-------|-----------|-------|-------|
| CLI Agent | `general-purpose` | containust-cli tests | Task #8: 2.1-2.10 |
| Core Agent | `general-purpose` | containust-core tests | Task #9: 3.1-3.16 |
| Runtime+VM Agent | `general-purpose` | runtime backend + vm cmds | Task #10: 4.1-4.6, 6.1-6.3 |
| Tests+Integration | `general-purpose` | SDK, TUI, eBPF, E2E | Task #11: 5.1-5.5, 7.1-7.3, 8.1-8.3, 9.1-9.3 |
| Infra Agent | `general-purpose` | CI/CD + deny | Task #12: 1.1-1.4 |
