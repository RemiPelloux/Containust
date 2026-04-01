# OPEN.md

This file provides guidance to OpenCode when working with code in this repository.

## Project Overview

**Containust** — a daemon-less, sovereign container runtime written in Rust (Edition 2024). Not a Docker clone; an evolution. Zero daemon, declarative `.ctst` composition language, native Linux isolation with macOS/Windows VM fallback via QEMU.

## Essential Commands

### Build & Check
```bash
cargo check --workspace              # Verify workspace compiles
cargo build --workspace              # Build all crates (debug)
cargo build --release -p containust-cli  # Production binary
cargo install --path crates/containust-cli  # Install `ctst` locally
```

### Test
```bash
cargo test --workspace               # All unit + integration tests
cargo test --workspace -- --test-threads=1  # If order-dependent
```
- Unit tests are co-located in `#[cfg(test)]` modules
- Integration tests live in `tests/integration/`
- Test naming: `<unit>_<scenario>_<expected_outcome>()`

### Lint & Format (zero warnings policy)
```bash
cargo clippy --workspace -- -D warnings  # Strict: pedantic + nursery lints
cargo fmt --workspace --check            # Formatting check
cargo deny check                         # Dependency audit
```
**All five checks must pass before any commit.** The project enforces `clippy::pedantic`, `clippy::nursery`, and bans `.unwrap()`, `.expect()`, `dbg!`, `print!`, `todo!`, and `panic!` in library code.

### Run Demo
```bash
ctst run examples/node-hello.ctst   # Hello World HTTP server on port 6500
ctst --version
ctst --help
```

## Architecture

### Crate Dependency DAG (9 crates, strictly layered)

```
CLI Layer        containust-cli (ctst binary) → containust-tui
SDK Layer        containust-sdk (public facade)
Engine Layer     containust-compose → containust-runtime → containust-image
Observe Layer    containust-ebpf
Core Layer       containust-core (Linux primitives)
Common Layer     containust-common (shared types/errors) — zero internal deps
```

| Crate | Responsibility |
|---|---|
| `containust-common` | Shared types, errors, config, constants. No algorithms, no I/O. |
| `containust-core` | Linux namespaces, cgroups v2, OverlayFS, capability dropping. All `unsafe` needs `// SAFETY:` comments. |
| `containust-image` | Image/layer management, source protocols (`file://`, `tar://`), SHA-256 verification |
| `containust-runtime` | Container lifecycle, state machine, process spawning, metrics. Implements `ContainerBackend` trait. |
| `containust-compose` | `.ctst` parser (nom), dependency graph (petgraph), auto-wiring resolution |
| `containust-ebpf` | eBPF syscall tracing, file/network monitoring (feature-gated) |
| `containust-sdk` | Public Rust SDK: `ContainerBuilder`, `GraphResolver`, `EventListener` |
| `containust-tui` | Interactive terminal dashboard (ratatui) |
| `containust-cli` | `ctst` binary with clap subcommands |

**Dependency rule**: Dependencies flow strictly downward. Upward or cross-layer dependencies are build-breaking. See ARCHITECTURE.md for the full matrix.

### Platform Backend Selection

| Platform | Backend | Mechanism |
|---|---|---|
| Linux | `LinuxNativeBackend` | Direct syscalls: `clone(2)`, `unshare(2)`, cgroups v2, OverlayFS |
| macOS/Windows | `VMBackend` | QEMU VM + JSON-RPC over TCP to BusyBox-based guest agent |

Backend auto-detected via `detect_backend()`. Platform code is gated with `#[cfg(target_os = "...")]`.

### Storage Model

Two-tier separation:
- **Global cache**: `~/.containust/cache/` — Immutable VM assets (kernel, initramfs), downloaded once
- **Project state**: `.containust/` (next to `.ctst` file) — Per-project container state, logs, images

### Error Handling

- **Library crates**: `thiserror` with crate-specific error enums. No `.unwrap()` allowed.
- **CLI crate**: `anyhow` for ergonomic propagation to user.
- Error messages must be actionable: `"failed to mount overlayfs at {path}: {source}"` not `"failed"`.

## Code Standards

### Limits
- Functions: max 25 lines
- Files: max 300 lines
- Module public items: max 10
- Function params: max 3 (use struct beyond that)
- No `utils.rs`, `helpers.rs`, or `misc.rs`

### Error Handling Rules
- Return `Result<T, E>` for all fallible operations
- No `.unwrap()` in library crates — use `.expect("reason")` only in tests
- No `panic!` in library code
- Every `unsafe` block requires `// SAFETY:` comment

### Naming
- Modules/files: `snake_case`
- Types: `PascalCase`
- Functions: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`

### Git & PRs
- Branch naming: `feat/`, `fix/`, `docs/`, `refactor/`, `test/`, `chore/`
- Commits: `<type>(<scope>): <summary>` (e.g., `fix(runtime): handle cgroup v2 delegation`)
- Coverage target: >=90% for library crates

## CI Pipeline

All checks run on every push/PR to `main`:
1. `cargo check --workspace` — compilation
2. `cargo fmt --workspace --check` — formatting
3. `cargo clippy --workspace -- -D warnings` — linting (zero warnings)
4. `cargo test --workspace` — unit + integration tests
5. `cargo deny check` — dependency security audit (weekly cron + PR)

Cross-platform matrix: Linux (full integration), macOS (compilation + unit tests), Windows (compilation + unit tests).

## Key Files for Orientation

| What to Read | Path |
|---|---|
| Full architecture | `ARCHITECTURE.md` |
| `.ctst` language reference | `docs/CTST_LANG.md` |
| CLI command manual | `docs/CLI_REFERENCE.md` |
| SDK usage guide | `docs/SDK_GUIDE.md` |
| Error code catalog | `docs/ERRORS.md` |
| Development & contribution guide | `docs/CONTRIBUTING.md` |
| Docker migration examples | `docs/MIGRATION_FROM_DOCKER.md` |
| Tutorial walkthroughs | `docs/TUTORIALS.md` |

## Examples Directory

| Example | Demonstrates |
|---|---|
| `examples/node-hello.ctst` | Working HTTP server demo |
| `examples/full_stack.ctst` | Multi-service with auto-wiring |
| `examples/microservices.ctst` | 5+ service dependency graph |
| `examples/offline_deployment.ctst` | Air-gapped with `tar://` sources |
| `examples/sdk_lifecycle.rs` | Container lifecycle via Rust SDK |
| `examples/templates/` | Reusable templates (PostgreSQL, Redis, nginx) |
