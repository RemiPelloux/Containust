# Architecture

This document describes the crate architecture, dependency rules, and design decisions of the Containust workspace.

## Crate Map

```
containust-cli ──────────► containust-sdk ──────────► containust-runtime
    │                          │    │                      │    │
    ▼                          │    │                      │    ▼
containust-tui ────────────────┘    │                      │ containust-ebpf
                                    ▼                      │    │
                             containust-compose            │    │
                                    │                      │    │
                                    ▼                      ▼    │
                             containust-core ◄─── containust-image
                                    │                      │
                                    ▼                      │
                             containust-common ◄───────────┘
```

## Layer Responsibilities

### Common Layer — `containust-common`

The foundation crate with zero internal dependencies. Contains:
- `error.rs` — Unified `ContainustError` enum (via `thiserror`).
- `types.rs` — Domain primitives: `ContainerId`, `ImageId`, `Sha256Hash`, `ResourceLimits`, `ContainerState`.
- `config.rs` — `ContainustConfig` model with default paths and runtime options.
- `constants.rs` — System paths, file extensions, limits, and application metadata.

**Rule**: No algorithms, no I/O, no business logic. Pure data definitions.

### Core Layer — `containust-core`

Safe abstractions over Linux kernel primitives:
- `namespace/` — PID, Mount, Network, User, IPC, UTS namespace creation and joining.
- `cgroup/` — Cgroups v2 resource management (CPU, memory, I/O) via `/sys/fs/cgroup`.
- `filesystem/` — OverlayFS mounting, `pivot_root(2)`, bind mounts, essential pseudo-fs setup.
- `capability.rs` — Linux capability dropping with allowlist semantics.

**Rule**: All `unsafe` blocks encapsulated in safe wrappers with `// SAFETY:` justification.

### Engine Layer

#### `containust-image`
Image and layer lifecycle:
- Content-addressed layer cache with SHA-256 verification.
- Local storage backend for on-disk image management.
- Source protocol handlers: `file://`, `tar://`, remote (opt-in).
- FUSE lazy-loading for fast container startup.

#### `containust-runtime`
Container lifecycle management:
- Container struct with state machine (Created → Running → Stopped → Failed).
- Process spawning inside isolated namespaces.
- Persistent state index (`state.json`) for daemon-less management.
- Namespace joining for `exec` operations.
- Real-time metrics collection from cgroup stat files.

#### `containust-compose`
`.ctst` language processing:
- Parser built on `nom`: lexer → AST → validator pipeline.
- Dependency graph construction and topological sorting via `petgraph`.
- Auto-wiring resolver for connection environment variables.
- IMPORT resolution from local and remote sources.
- Distroless analyzer using ELF binary dependency scanning.

### Observability Layer — `containust-ebpf`

eBPF-based container monitoring (optional, feature-gated):
- Syscall tracing via tracepoints.
- File open monitoring.
- Network socket/connection tracking.
- eBPF programs loaded via `aya`.

### SDK Layer — `containust-sdk`

Public facade for using Containust as a library:
- `ContainerBuilder` — Fluent API for container configuration and launch.
- `GraphResolver` — High-level `.ctst` loading and dependency resolution.
- `EventListener` — Async event stream for lifecycle monitoring.

**Rule**: The SDK is the only crate that external consumers should depend on directly.

### CLI Layer

#### `containust-cli`
The `ctst` binary with subcommands: `build`, `plan`, `run`, `ps`, `exec`, `stop`, `images`.
Uses `clap` for argument parsing and `anyhow` for error reporting.

#### `containust-tui`
Interactive terminal dashboard built with `ratatui`:
- Dashboard view with container table.
- Container detail view with config and live metrics.
- eBPF trace log viewer.

## Dependency Rules

Dependencies flow strictly downward through the layers. The complete allowed-dependency table:

| Crate | Allowed Internal Dependencies |
|---|---|
| `containust-common` | None |
| `containust-core` | `containust-common` |
| `containust-image` | `containust-common`, `containust-core` |
| `containust-runtime` | `containust-common`, `containust-core`, `containust-ebpf` |
| `containust-compose` | `containust-common`, `containust-core` |
| `containust-ebpf` | `containust-common` |
| `containust-sdk` | `containust-common`, `containust-runtime`, `containust-image`, `containust-compose` |
| `containust-tui` | `containust-common`, `containust-sdk` |
| `containust-cli` | `containust-common`, `containust-sdk`, `containust-tui` |

**Violations of this table are build-breaking errors.**

## Design Decisions

### Why no daemon?
Traditional container runtimes use a root daemon for lifecycle management. Containust replaces this with a state file (`state.json`) and direct syscalls, eliminating a permanent attack surface.

### Why `pivot_root` over `chroot`?
`chroot` only changes the process's view of `/` — the old root remains accessible. `pivot_root` actually moves the root mount point, providing stronger isolation.

### Why OverlayFS?
OverlayFS enables efficient layer caching and copy-on-write semantics without duplicating filesystem data. Combined with content-addressed storage, this minimizes disk usage.

### Why feature-gated eBPF?
eBPF requires a modern Linux kernel and BPF support. Feature-gating it with `ebpf` allows the core runtime to work on systems without BPF, including development on macOS via cross-compilation.
