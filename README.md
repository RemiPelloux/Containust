# Containust

**Daemon-less, sovereign container runtime written in Rust.**

Containust combines Linux isolation primitives with Infrastructure-as-Code composition in a single secure binary. No root daemon. No background processes. Full sovereignty over your infrastructure.

---

## Vision

Containust is not a Docker clone. It is an evolution that prioritizes:

- **Zero Daemon** — No permanent root process. Containers are managed through a state file and direct system calls.
- **Native Composition** — Dependency graphs between containers are first-class citizens via the `.ctst` language.
- **Sovereignty** — Local-first design with native `file://` and `tar://` source protocols. Air-gapped environments are fully supported.
- **Security by Default** — Read-only rootfs, capability dropping, and strict offline mode out of the box.

---

## Architecture

The project is organized as a Cargo workspace with 9 specialized crates arranged in a strict dependency DAG:

```
CLI Layer        containust-cli (ctst binary), containust-tui
                        |
SDK Layer        containust-sdk (public facade)
                        |
Engine Layer     containust-compose, containust-runtime, containust-image
                        |
Observe Layer    containust-ebpf
                        |
Core Layer       containust-core (namespaces, cgroups, filesystem)
                        |
Common Layer     containust-common (types, errors, constants)
```

| Crate | Purpose |
|---|---|
| `containust-common` | Shared types, error definitions, configuration, constants |
| `containust-core` | Linux namespace, cgroup v2, OverlayFS, and capability primitives |
| `containust-image` | Image/layer management, storage, source protocols, SHA-256 validation |
| `containust-runtime` | Container lifecycle, process spawning, state machine, metrics |
| `containust-compose` | `.ctst` parser (nom), dependency graph (petgraph), auto-wiring |
| `containust-ebpf` | eBPF-based syscall/file/network monitoring (aya) |
| `containust-sdk` | Public SDK: ContainerBuilder, GraphResolver, EventListener |
| `containust-tui` | Interactive terminal dashboard (ratatui) |
| `containust-cli` | `ctst` binary with all subcommands (clap) |

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full crate dependency graph and design decisions.

---

## Quick Start

### Prerequisites

- **Rust 1.85+** (Edition 2024)
- **Linux kernel 5.10+** (for cgroups v2, user namespaces, OverlayFS)
- **Optional**: eBPF support for observability features

### Build

```bash
# Clone the repository
git clone https://github.com/containust/containust.git
cd containust

# Build the entire workspace
cargo build --workspace

# Build release binary
cargo build --release -p containust-cli

# The binary is at target/release/ctst
```

### Install

```bash
cargo install --path crates/containust-cli
```

### Verify

```bash
ctst --version
ctst --help
```

---

## CLI Reference

| Command | Description |
|---|---|
| `ctst build` | Parse a `.ctst` file and generate images/layers |
| `ctst plan` | Display planned infrastructure changes before applying |
| `ctst run` | Deploy the component graph |
| `ctst ps` | List containers with real-time metrics |
| `ctst exec` | Execute a command inside a running container |
| `ctst stop` | Stop containers and clean up resources |
| `ctst images` | Manage the local image catalog |

### Global Flags

| Flag | Description |
|---|---|
| `--offline` | Block all outbound network access (build and run) |
| `--state-file <path>` | Custom path to the state index file |

See [docs/CLI_REFERENCE.md](docs/CLI_REFERENCE.md) for detailed usage examples.

---

## The .ctst Composition Language

Containust uses a declarative, LLM-friendly composition language:

```ctst
IMPORT "base/postgres.ctst" AS db_template

COMPONENT api {
    image = "file:///opt/images/myapp"
    port = 8080
    env = {
        DATABASE_URL = "${db.connection_string}"
        RUST_LOG = "info"
    }
}

COMPONENT db FROM db_template {
    port = 5432
    volume = "/data/postgres:/var/lib/postgresql/data"
    memory = "512MB"
}

CONNECT api -> db
```

Key features:
- **IMPORT** — Compose from other `.ctst` files or remote sources.
- **COMPONENT** — Define reusable, parameterized building blocks.
- **CONNECT** — Declare dependencies with automatic environment variable injection.
- **Distroless Analysis** — Automatic binary dependency scanning for minimal images.

See [docs/CTST_LANG.md](docs/CTST_LANG.md) for the full language specification.

---

## SDK Usage

Use Containust as a Rust library:

```rust
use containust_sdk::builder::ContainerBuilder;

fn main() -> anyhow::Result<()> {
    let container = ContainerBuilder::new("my-service")
        .image("file:///opt/images/alpine")
        .command(vec!["./server".into()])
        .env("PORT", "8080")
        .memory_limit(128 * 1024 * 1024)  // 128 MiB
        .cpu_shares(1024)
        .build()?;

    // container.start()?;
    Ok(())
}
```

See [docs/SDK_GUIDE.md](docs/SDK_GUIDE.md) for the full SDK documentation.

---

## Security Model

| Feature | Default |
|---|---|
| Root filesystem | Read-only |
| Linux capabilities | All dropped, allowlist only |
| Network (offline mode) | All egress blocked |
| Image sources | Local-first (`file://`, `tar://`) |
| Content verification | SHA-256 mandatory |
| State file | No credentials stored |

---

## Observability

### TUI Dashboard

```bash
ctst ps --tui
```

Interactive terminal dashboard powered by ratatui showing:
- Container status and uptime
- Real-time CPU, memory, and I/O metrics
- eBPF trace logs (syscalls, file access, network events)

### eBPF Tracing

When built with the `ebpf` feature and running on a supported kernel:
- Syscall tracing per container
- File open monitoring
- Network socket/connection tracking

---

## Development

```bash
# Run all tests
cargo test --workspace

# Check lints
cargo clippy --workspace -- -D warnings

# Format code
cargo fmt --workspace

# Audit dependencies
cargo deny check

# Check the full workspace compiles
cargo check --workspace
```

---

## Project Structure

```
containust/
├── Cargo.toml              # Workspace manifest
├── crates/                 # All library and binary crates
│   ├── containust-common/  # Shared types, errors, constants
│   ├── containust-core/    # Linux primitives
│   ├── containust-image/   # Image/layer management
│   ├── containust-runtime/ # Container lifecycle
│   ├── containust-compose/ # .ctst parser + graph
│   ├── containust-ebpf/    # eBPF observability
│   ├── containust-sdk/     # Public SDK
│   ├── containust-tui/     # Terminal dashboard
│   └── containust-cli/     # ctst binary
├── docs/                   # Documentation
├── tests/integration/      # Integration tests
└── examples/               # Example .ctst files and SDK usage
```

---

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
