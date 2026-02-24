<p align="center">
  <h1 align="center">Containust</h1>
  <p align="center">
    <strong>Daemon-less, sovereign container runtime written in Rust</strong>
  </p>
  <p align="center">
    A next-generation container engine — zero daemon, native composition, air-gap ready. Cross-platform: Linux, macOS, Windows.
  </p>
</p>

<p align="center">
  <a href="https://github.com/RemiPelloux/Containust/actions/workflows/ci.yml"><img src="https://github.com/RemiPelloux/Containust/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/RemiPelloux/Containust/actions/workflows/security.yml"><img src="https://github.com/RemiPelloux/Containust/actions/workflows/security.yml/badge.svg" alt="Security"></a>
  <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/License-MIT%2FApache--2.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-Edition%202024-orange.svg?logo=rust" alt="Rust"></a>
  <a href="README.md"><img src="https://img.shields.io/badge/Linux-native-success?logo=linux" alt="Linux"></a>
  <a href="README.md"><img src="https://img.shields.io/badge/macOS-VM_backend-blue?logo=apple" alt="macOS"></a>
  <a href="README.md"><img src="https://img.shields.io/badge/Windows-VM_backend-blue?logo=windows" alt="Windows"></a>
  <img src="https://img.shields.io/badge/daemon-zero-brightgreen.svg" alt="Zero Daemon">
  <img src="https://img.shields.io/badge/tests-167_passing-success" alt="Tests">
</p>

---

## What is Containust?

Containust is **not a Docker clone** — it is an evolution of container technology. It combines the power of Linux isolation (namespaces, cgroups v2, OverlayFS) with Infrastructure-as-Code composition (Terraform-style) in a **single secure binary with no daemon**.

Built entirely in Rust for **memory safety, performance, and reliability**, Containust targets sovereign infrastructures, air-gapped environments, and security-critical deployments where a permanent root daemon is unacceptable. It runs **natively on Linux** with zero overhead, and on **macOS/Windows** via a lightweight QEMU-backed VM with near-native performance.

### Why Containust?

| Problem | Containust's Answer |
|---|---|
| Docker requires a root daemon | **Zero daemon** — direct syscalls, state file |
| Compose files are imperative | **Declarative `.ctst` language** with dependency graphs |
| Online-only image pulling | **Local-first** — `file://`, `tar://` protocols, air-gap native |
| Opaque container behavior | **eBPF tracing** — real-time syscall/file/network monitoring |
| Monolithic tooling | **9 modular crates** — use as CLI or embed as Rust SDK |

---

## Platform Support

| Platform | Backend | Performance | Requirements |
|----------|---------|-------------|--------------|
| **Linux** | Native (direct syscalls) | Zero overhead | Linux 5.10+, cgroups v2 |
| **macOS** | Lightweight VM via QEMU | Near-native | QEMU (`brew install qemu`) |
| **Windows** | Lightweight VM via QEMU | Near-native | QEMU (via `winget` or installer) |

---

## How It Works

Containust uses a **native-first, VM-fallback** architecture:

- **On Linux**: Direct kernel integration using namespaces (`clone(2)`, `unshare(2)`), cgroups v2, `OverlayFS`, and `pivot_root`. Zero overhead, zero daemon.
- **On macOS/Windows**: A lightweight Alpine Linux VM (~50MB) boots via QEMU with hardware acceleration (HVF on macOS, Hyper-V/WHPX on Windows). Container operations are forwarded to the Linux native backend inside the VM via JSON-RPC over TCP. Sub-2s boot time.

This is the same architecture used by Docker Desktop, Podman Desktop, and Colima — but Containust is a **single binary with no daemon**.

---

## Key Features

- **Zero Daemon Architecture** — No persistent root process. Containers managed via state file and direct Linux syscalls.
- **Native Composition Language** — `.ctst` declarative format with `IMPORT`, `COMPONENT`, `CONNECT`, and automatic environment wiring.
- **Sovereign & Air-Gap Ready** — Priority to local sources (`file://`, `tar://`). `--offline` flag blocks all network egress.
- **Security by Default** — Read-only rootfs, Linux capability dropping, SHA-256 content verification.
- **eBPF Observability** — Real-time syscall tracing, file access monitoring, network socket tracking inside containers.
- **Interactive TUI Dashboard** — Terminal-based monitoring with live CPU, memory, I/O metrics via ratatui.
- **Rust SDK** — Embed container management in your Rust applications with `ContainerBuilder`, `GraphResolver`, `EventListener`.
- **Distroless Auto-Build** — Automatic binary dependency analysis (internal ldd) for minimal container images.

---

## Architecture

The project is organized as a **Cargo workspace with 9 specialized crates** arranged in a strict layered dependency DAG:

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

| Crate | Responsibility |
|---|---|
| `containust-common` | Shared types, error definitions, configuration, constants |
| `containust-core` | Linux namespace, cgroup v2, OverlayFS, and capability primitives |
| `containust-image` | Image/layer management, storage, source protocols, SHA-256 validation |
| `containust-runtime` | Container lifecycle, process spawning, state machine, metrics |
| `containust-compose` | `.ctst` parser (nom), dependency graph (petgraph), auto-wiring |
| `containust-ebpf` | eBPF-based syscall/file/network monitoring (aya) |
| `containust-sdk` | Public SDK: `ContainerBuilder`, `GraphResolver`, `EventListener` |
| `containust-tui` | Interactive terminal dashboard (ratatui) |
| `containust-cli` | `ctst` binary with all subcommands (clap) |

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full dependency graph and design decisions.

---

## Quick Start

### Prerequisites

**Linux:**
```bash
# No additional dependencies needed
# Requires Linux kernel 5.10+ (cgroups v2, user namespaces, OverlayFS)
# Optional: kernel 5.15+ for eBPF observability features
curl -sSL https://github.com/containust/containust/releases/latest/download/ctst-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv ctst /usr/local/bin/
```

**macOS:**
```bash
brew install qemu
curl -sSL https://github.com/containust/containust/releases/latest/download/ctst-aarch64-apple-darwin.tar.gz | tar xz
sudo mv ctst /usr/local/bin/
```

**Windows:**
```powershell
winget install QEMU.QEMU
# Download ctst from GitHub Releases
```

**Build from Source (all platforms):**

- **Rust 1.85+** (Edition 2024)

### Build from Source

```bash
git clone https://github.com/RemiPelloux/Containust.git
cd Containust

# Build the entire workspace
cargo build --workspace

# Build optimized release binary
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

The `ctst` command is the single entry point for all container operations:

| Command | Description |
|---|---|
| `ctst build` | Parse a `.ctst` file and generate images/layers |
| `ctst plan` | Display planned infrastructure changes before applying |
| `ctst run` | Deploy the component graph |
| `ctst ps` | List containers with real-time metrics |
| `ctst exec` | Execute a command inside a running container |
| `ctst stop` | Stop containers and clean up resources |
| `ctst logs` | View container logs (`--follow` for live tailing) |
| `ctst images` | Manage the local image catalog |
| `ctst convert` | Convert a `docker-compose.yml` to `.ctst` format |
| `ctst vm start` | Start the platform VM (macOS/Windows only) |
| `ctst vm stop` | Stop the platform VM |

### Global Flags

| Flag | Description |
|---|---|
| `--offline` | Block all outbound network access (build and run) |
| `--state-file <path>` | Custom path to the state index file |

See [docs/CLI_REFERENCE.md](docs/CLI_REFERENCE.md) for the complete CLI manual with output formats, exit codes, and troubleshooting.

---

## Documentation

| Document | Description |
|---|---|
| [`.ctst` Language Reference](docs/CTST_LANG.md) | Complete language specification — all keywords, types, syntax, protocols, and examples |
| [CLI Reference](docs/CLI_REFERENCE.md) | Full `ctst` command manual — every flag, output format, exit code, troubleshooting |
| [SDK Guide](docs/SDK_GUIDE.md) | Rust SDK API reference — `ContainerBuilder`, `GraphResolver`, `EventListener`, types, errors |
| [Tutorials](docs/TUTORIALS.md) | 9 step-by-step tutorials from Hello World to SDK integration |
| [Error Reference](docs/ERRORS.md) | Catalog of all error codes (parse, runtime, image, state) with causes and resolutions |
| [Migration from Docker](docs/MIGRATION_FROM_DOCKER.md) | Docker Compose to `.ctst` conversion guide with side-by-side examples |
| [Contributing Guide](docs/CONTRIBUTING.md) | Development setup, coding standards, how to add commands/keywords/backends |
| [Architecture](ARCHITECTURE.md) | Crate dependency DAG, layer responsibilities, design decisions |
| [Specification](docs/SPEC.md) | Technical specification — vision, engine design, security model |

### Examples

The `examples/` directory contains ready-to-use `.ctst` composition files and Rust SDK examples:

| Example | What it demonstrates |
|---|---|
| [`hello_world.ctst`](examples/hello_world.ctst) | Minimal single container |
| [`nginx_static.ctst`](examples/nginx_static.ctst) | Web server with volume mount and port exposure |
| [`full_stack.ctst`](examples/full_stack.ctst) | API + PostgreSQL + Redis + worker with `CONNECT` auto-wiring |
| [`microservices.ctst`](examples/microservices.ctst) | 5+ services with complex dependency graph |
| [`offline_deployment.ctst`](examples/offline_deployment.ctst) | Air-gapped deployment with `tar://` sources |
| [`secrets_example.ctst`](examples/secrets_example.ctst) | Secret injection patterns |
| [`healthcheck_example.ctst`](examples/healthcheck_example.ctst) | Health checks and restart policies |
| [`templates/`](examples/templates/) | Reusable templates (PostgreSQL, Redis, nginx) |
| [`sdk_lifecycle.rs`](examples/sdk_lifecycle.rs) | Container lifecycle via Rust SDK |
| [`sdk_composition.rs`](examples/sdk_composition.rs) | Loading `.ctst` files via SDK |
| [`sdk_monitoring.rs`](examples/sdk_monitoring.rs) | Event monitoring via SDK |

---

## The `.ctst` Composition Language

Containust uses a **declarative, LLM-friendly composition language** designed for Infrastructure-as-Code:

```
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

**Language features:**
- **IMPORT** — Compose from other `.ctst` files or remote sources.
- **COMPONENT** — Define reusable, parameterized building blocks.
- **CONNECT** — Declare dependencies with automatic environment variable injection.
- **Distroless Analysis** — Automatic binary dependency scanning for minimal images.

See [docs/CTST_LANG.md](docs/CTST_LANG.md) for the full language specification.

---

## Rust SDK

Use Containust as an embeddable Rust library:

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

Security is not an afterthought — it is the foundation:

| Feature | Default | Description |
|---|---|---|
| Root filesystem | **Read-only** | Only declared volumes are writable |
| Linux capabilities | **All dropped** | Allowlist-only model |
| Network (offline) | **All egress blocked** | `--offline` flag for air-gapped builds and runs |
| Image sources | **Local-first** | `file://`, `tar://` protocols with SHA-256 verification |
| State file | **No secrets** | Credentials never stored in state index |
| Unsafe code | **Audited** | Every `unsafe` block requires `// SAFETY:` justification |

---

## Observability

### TUI Dashboard

```bash
ctst ps --tui
```

Interactive terminal dashboard powered by **ratatui** showing:
- Container status and uptime
- Real-time CPU, memory, and I/O metrics
- eBPF trace logs (syscalls, file access, network events)

### eBPF Tracing

When built with the `ebpf` feature and running on a supported kernel:
- Syscall tracing per container
- File open monitoring
- Network socket/connection tracking

---

## Tech Stack

| Component | Technology |
|---|---|
| Language | Rust (Edition 2024) |
| CLI | clap 4.5 |
| TUI | ratatui 0.30 |
| Parsing | nom 8 |
| Dependency Graph | petgraph 0.7 |
| Linux Syscalls | nix 0.29, libc |
| eBPF | aya 0.13 |
| Serialization | serde, serde_json |
| Hashing | sha2 (SHA-256) |
| Logging | tracing |

---

## Development

```bash
# Run all tests
cargo test --workspace

# Check lints (zero warnings policy)
cargo clippy --workspace -- -D warnings

# Format code
cargo fmt --all

# Audit dependencies
cargo deny check

# Verify workspace compiles
cargo check --workspace
```

---

## Project Structure

```
Containust/
├── Cargo.toml                  # Workspace manifest
├── crates/                     # All library and binary crates
│   ├── containust-common/      # Shared types, errors, constants
│   ├── containust-core/        # Linux isolation primitives
│   ├── containust-image/       # Image/layer management
│   ├── containust-runtime/     # Container lifecycle
│   │   └── src/
│   │       ├── backend/        # Platform-agnostic backends
│   │       │   ├── mod.rs      # ContainerBackend trait
│   │       │   ├── linux.rs    # LinuxNativeBackend (direct syscalls)
│   │       │   └── vm.rs       # VMBackend (QEMU + JSON-RPC)
│   │       ├── engine.rs       # Runtime engine (orchestrates deployments)
│   │       └── logs.rs         # Container log management
│   ├── containust-compose/     # .ctst parser + dependency graph
│   ├── containust-ebpf/        # eBPF observability
│   ├── containust-sdk/         # Public Rust SDK
│   ├── containust-tui/         # Terminal dashboard
│   └── containust-cli/         # ctst binary
│       └── src/commands/
│           ├── logs.rs         # ctst logs command
│           └── converter.rs    # Docker Compose converter
├── docs/                       # Documentation
│   ├── CTST_LANG.md            # .ctst language reference
│   ├── CLI_REFERENCE.md        # CLI manual
│   ├── SDK_GUIDE.md            # Rust SDK guide
│   ├── TUTORIALS.md            # Step-by-step tutorials
│   ├── ERRORS.md               # Error code reference
│   ├── CONTRIBUTING.md         # Contributor guide
│   ├── MIGRATION_FROM_DOCKER.md # Docker Compose migration
│   └── SPEC.md                 # Technical specification
├── examples/                   # Example files
│   ├── templates/              # Reusable .ctst templates
│   ├── *.ctst                  # Composition examples
│   └── *.rs                    # Rust SDK examples
├── tests/integration/          # Integration tests
└── ARCHITECTURE.md             # Crate architecture
```

---

## Contributing

Contributions are welcome. Please read the [Contributing Guide](docs/CONTRIBUTING.md) before submitting code.

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/my-feature`)
3. Ensure `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, and `cargo fmt --all --check` pass
4. Submit a pull request

See also: [ARCHITECTURE.md](ARCHITECTURE.md) | [Error Reference](docs/ERRORS.md) | [Cursor rules](.cursor/rules/)

---

## License

Licensed under either of:

- [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
- [MIT License](http://opensource.org/licenses/MIT)

at your option.

---

<p align="center">
  Built with Rust. Designed for sovereignty.
</p>
