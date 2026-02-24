# Contributing to Containust

Welcome, and thank you for your interest in contributing to **Containust** — a daemon-less, sovereign container runtime written in Rust.

This guide covers everything you need to go from zero to merged pull request. We value code that is **clear, modular, secure, and well-tested**. Every contribution, no matter how small, makes Containust better.

---

## Getting Started

### Prerequisites

| Tool | Minimum Version | Purpose |
|---|---|---|
| **Rust** | 1.85+ (Edition 2024, stable channel) | Build and test |
| **Git** | 2.x | Version control |
| **Linux** | Kernel 5.10+ | Runtime (namespaces, cgroups v2, OverlayFS) |
| **macOS** | 13+ *(optional)* | Compilation only (runtime requires Linux) |
| **cargo-deny** | latest | Dependency auditing |

### Clone and Build

```bash
git clone https://github.com/RemiPelloux/Containust.git
cd Containust

cargo build --workspace
```

### Run Tests

```bash
cargo test --workspace
```

### Verify Lints Pass

The project enforces a **zero warnings** policy with `clippy::pedantic` and `clippy::nursery`:

```bash
cargo clippy --workspace -- -D warnings
cargo fmt --workspace --check
cargo deny check
```

All three commands must pass before submitting a pull request.

---

## Project Architecture

### Crate Dependency Graph

```
                   ┌──────────────┐
                   │ containust-  │
                   │     cli      │  ← binary: ctst
                   └──────┬───────┘
                          │
                   ┌──────┴───────┐
                   │ containust-  │
                   │     tui      │
                   └──────┬───────┘
                          │
                   ┌──────┴───────┐
                   │ containust-  │
                   │     sdk      │  ← public API facade
                   └──┬───┬───┬──┘
                      │   │   │
         ┌────────────┘   │   └────────────┐
         ▼                ▼                ▼
  ┌────────────┐   ┌────────────┐   ┌────────────┐
  │ containust-│   │ containust-│   │ containust-│
  │   compose  │   │  runtime   │   │   image    │
  └─────┬──────┘   └─────┬──────┘   └─────┬──────┘
        │                │                 │
        │          ┌─────┴──────┐          │
        │          │ containust-│          │
        │          │    ebpf    │          │
        │          └─────┬──────┘          │
        │                │                 │
        └────────┬───────┴────────┬────────┘
                 ▼                ▼
          ┌────────────┐   (also depends on)
          │ containust-│
          │    core    │
          └─────┬──────┘
                │
                ▼
          ┌────────────┐
          │ containust-│
          │   common   │  ← leaf crate, zero internal deps
          └────────────┘
```

### Crate Responsibilities

| Crate | Layer | Responsibility |
|---|---|---|
| `containust-common` | Common | Shared types, error definitions, configuration models, constants |
| `containust-core` | Core | Linux namespace, cgroup v2, OverlayFS, and capability primitives |
| `containust-image` | Engine | Image/layer storage, source protocols (`file://`, `tar://`, `https://`), SHA-256 validation |
| `containust-runtime` | Engine | Container lifecycle state machine, process spawning, metrics collection |
| `containust-compose` | Engine | `.ctst` parser (nom), dependency graph (petgraph), auto-wiring logic |
| `containust-ebpf` | Observe | eBPF-based syscall, file access, and network monitoring (aya) |
| `containust-sdk` | SDK | Public facade: `ContainerBuilder`, `GraphResolver`, `EventListener` |
| `containust-tui` | CLI | Interactive terminal dashboard (ratatui) |
| `containust-cli` | CLI | `ctst` binary with all subcommands (clap) |

### Layer Rules

Dependencies flow **strictly downward**. A crate may only depend on crates in its own layer or lower layers. Cross-layer or upward dependencies are forbidden. See [architecture.mdc](../.cursor/rules/architecture.mdc) for the full dependency matrix.

---

## Coding Standards

### Naming Conventions

| Element | Convention | Example |
|---|---|---|
| Modules and files | `snake_case` | `pivot_root.rs`, `file_monitor.rs` |
| Types (structs, enums, traits) | `PascalCase` | `ContainerState`, `LayerHash` |
| Functions and methods | `snake_case` | `create_namespace`, `resolve_graph` |
| Constants | `SCREAMING_SNAKE_CASE` | `DEFAULT_CGROUP_PATH` |
| Type parameters | Single uppercase or short `PascalCase` | `T`, `Err` |

### Function Rules

- **Single responsibility** — each function does one thing.
- **Max 25 lines** per function body. Extract helpers if longer.
- **Max 3 parameters** — beyond 3, use a config/options struct.
- **Early returns** — use guard clauses to reduce nesting.
- **Prefer pure functions** — isolate side effects (I/O, syscalls) at module boundaries.
- **Return `Result<T, E>`** for all fallible operations.

### Module Rules

- **Single concern per file** — `storage.rs` handles storage, not networking.
- **Max 300 lines per file** — split into submodules when approaching this limit.
- **Max 10 public items per module** — a module exposing more is doing too much.
- **No catch-all modules** — `utils.rs`, `helpers.rs`, and `misc.rs` are banned.

### Error Handling

- **No `.unwrap()` in library crates** — use `.expect("reason")` only in tests.
- **No `panic!` in library code** — reserve panics for truly unrecoverable states.
- **Library crates use `thiserror`** — each crate defines its own error enum.
- **The CLI crate uses `anyhow`** — for ergonomic error propagation to the user.
- **Error messages must be actionable** — bad: `"failed"`; good: `"failed to mount overlayfs at {path}: {source}"`.

### Unsafe Code

- Every `unsafe` block requires a `// SAFETY:` comment explaining invariant upholding.
- Minimize unsafe scope — wrap in safe abstractions as close to the call site as possible.
- No `unsafe` in public APIs — encapsulate in safe wrappers that validate preconditions.

### Security

- Read-only rootfs by default — only declared volumes are writable.
- Drop all Linux capabilities — allowlist only what is required, with justification comments.
- No secrets in state files or logs — scrub sensitive data from all output.
- Validate all external input — file paths, image URIs, and `.ctst` files are checked before use.

---

## How to Add a New CLI Command

1. **Define the subcommand** in `crates/containust-cli/src/commands/mod.rs` by adding a new variant to the `Commands` enum (clap derive).

2. **Create the handler module** at `crates/containust-cli/src/commands/<name>.rs`. The handler should accept parsed arguments and call into `containust-sdk`.

3. **Wire the handler** into the `match` block in `commands/mod.rs` that dispatches on the `Commands` enum.

4. **Add tests**:
   - Unit test in the handler module (`#[cfg(test)] mod tests { ... }`).
   - Integration test in `tests/integration/` exercising the command end-to-end.

5. **Update documentation** — add the command to `docs/CLI_REFERENCE.md` and `README.md`.

---

## How to Add a New `.ctst` Keyword

1. **Add the keyword token** to the lexer/tokenizer in `crates/containust-compose/src/parser/`. Update the reserved keywords list.

2. **Update the AST types** in `crates/containust-compose/src/ast.rs` to represent the new construct.

3. **Implement the parser rule** using nom combinators. Follow the existing parsing patterns for consistency.

4. **Add semantic analysis** — validate the new keyword's constraints (type checks, reference resolution) in the analysis pass.

5. **Add tests**:
   - Parser unit test: valid syntax parses correctly.
   - Parser unit test: invalid syntax produces `E001` or appropriate error.
   - Semantic test: constraints are enforced (references exist, types match).

6. **Update documentation** — add the keyword to `docs/CTST_LANG.md` and the reserved keywords table.

---

## How to Add a New Container Backend

1. **Define the trait** (if not already present) in `crates/containust-core/src/`. The trait should abstract namespace creation, filesystem setup, and process spawning.

2. **Implement the trait** in a new module within `containust-core`. Each method should return `Result<T, E>` with descriptive errors.

3. **Add platform detection** — use `#[cfg(target_os = "...")]` or runtime feature detection to select the appropriate backend.

4. **Register the backend** in the engine initialization code so that `containust-runtime` can discover and use it.

5. **Add tests**:
   - Unit tests with mocked system calls.
   - Integration test in `tests/integration/` using the real backend (Linux only).

6. **Gate behind a feature flag** if the backend introduces heavy dependencies (e.g., eBPF, FUSE).

---

## Testing Strategy

### Unit Tests

- Co-located with source code inside `#[cfg(test)] mod tests { ... }`.
- One assertion per test when feasible.
- Must be deterministic — no flaky tests. Mock external dependencies.
- Must complete in **< 100ms** individually.

### Integration Tests

- Located in `tests/integration/`.
- May touch real namespaces, cgroups, and filesystems.
- Must complete in **< 5 seconds** individually.
- Use `tempfile` for filesystem operations — never write to fixed paths.

### Scenario Tests

- End-to-end tests exercising `ctst` subcommands with real `.ctst` files.
- Located in `tests/integration/` alongside other integration tests.
- Validate the full pipeline: parse → build → run → verify → stop.

### Coverage Target

- **>=90% line coverage** for library crates.
- CLI/TUI crates may have lower coverage due to I/O-heavy code.

### Test Naming

```
#[test]
fn <unit>_<scenario>_<expected_outcome>()
```

Examples:
- `fn container_with_invalid_image_returns_error()`
- `fn parser_empty_input_produces_empty_ast()`
- `fn cgroup_memory_limit_enforced_correctly()`

---

## CI Pipeline

### `ci.yml` — Continuous Integration

Runs on every push and pull request to `main`:

| Job | Command | Purpose |
|---|---|---|
| **Check** | `cargo check --workspace` | Verify the workspace compiles |
| **Format** | `cargo fmt --workspace --check` | Enforce consistent formatting |
| **Clippy** | `cargo clippy --workspace -- -D warnings` | Zero-warning lint policy |
| **Test** | `cargo test --workspace` | Run all unit and integration tests |

### `security.yml` — Dependency Audit

Runs on push, pull request, and weekly schedule (Monday 06:00 UTC):

| Job | Command | Purpose |
|---|---|---|
| **Audit** | `cargo deny check` | Check for known vulnerabilities and license compliance |

### Branch Protection

- All CI jobs must pass before merge.
- At least one approving review is required.
- Force pushes to `main` are disabled.

---

## Pull Request Process

### Branch Naming

| Prefix | Purpose | Example |
|---|---|---|
| `feat/` | New feature | `feat/healthcheck-support` |
| `fix/` | Bug fix | `fix/cgroup-path-resolution` |
| `docs/` | Documentation only | `docs/sdk-guide-examples` |
| `refactor/` | Code restructuring (no behavior change) | `refactor/parser-combinator-cleanup` |
| `test/` | Test additions or fixes | `test/namespace-edge-cases` |
| `chore/` | Tooling, CI, dependencies | `chore/update-clap-to-4.5` |

### Commit Message Format

```
<type>(<scope>): <short summary>

<optional body explaining why, not what>
```

Examples:
- `feat(compose): add HEALTHCHECK keyword to .ctst parser`
- `fix(runtime): handle cgroup v2 delegation correctly on systemd hosts`
- `docs(sdk): add ContainerBuilder usage examples`

### Required Checks

Before requesting review, ensure:

- [ ] `cargo check --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] `cargo fmt --workspace --check` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo deny check` passes
- [ ] New code has unit tests (>=90% coverage for library crates)
- [ ] Documentation is updated if public API changed

### Review Criteria

Reviewers evaluate contributions on:

1. **Correctness** — does the code do what it claims?
2. **Architecture** — does it respect the layer DAG and crate boundaries?
3. **Modularity** — functions <=25 lines, files <=300 lines, no god-objects?
4. **Security** — no `.unwrap()` in libraries, unsafe blocks justified, inputs validated?
5. **Tests** — meaningful coverage, deterministic, fast?
6. **Documentation** — public items have doc comments, error messages are actionable?

---

## Release Process

### Versioning

Containust follows [Semantic Versioning](https://semver.org/):

- **MAJOR** — breaking changes to the SDK public API or `.ctst` language syntax.
- **MINOR** — new features, new CLI commands, new `.ctst` keywords (backward-compatible).
- **PATCH** — bug fixes, performance improvements, documentation updates.

### Changelog

Maintain `CHANGELOG.md` in [Keep a Changelog](https://keepachangelog.com/) format:

```markdown
## [Unreleased]

### Added
- HEALTHCHECK support in .ctst language (#42)

### Fixed
- Cgroup path resolution on systemd-managed hosts (#38)

### Changed
- Upgraded petgraph from 0.6 to 0.7 (#41)
```

### Tag and Publish

```bash
# Update version in all Cargo.toml files
cargo set-version --workspace 0.2.0

# Update CHANGELOG.md — move [Unreleased] to [0.2.0]

# Commit and tag
git add -A
git commit -m "release: v0.2.0"
git tag v0.2.0

# Push
git push origin main --tags
```

---

## Getting Help

- **Issues**: Open an issue on GitHub for bugs, feature requests, or questions.
- **Architecture**: Read [ARCHITECTURE.md](../ARCHITECTURE.md) for design rationale.
- **Language spec**: Read [CTST_LANG.md](CTST_LANG.md) for the `.ctst` format reference.
- **Error codes**: Read [ERRORS.md](ERRORS.md) for the full error catalog.
- **Cursor rules**: Browse `.cursor/rules/` for enforced coding standards.

---

*Built with Rust. Designed for sovereignty.*
