# Containust

Containust is a daemon-less container runtime and declarative composition tool written in Rust. It is designed for local, sovereign, and air-gapped workflows where a long-running privileged daemon is undesirable.

> **Project status: alpha (0.1.0).** The parser, dependency graph, local image primitives, state/log persistence, CLI parsing, and unit/integration test suite are working. Native Linux process isolation and the QEMU VM backend are implemented but require privileged, platform-specific validation before production use.

[![CI](https://github.com/RemiPelloux/Containust/actions/workflows/ci.yml/badge.svg)](https://github.com/RemiPelloux/Containust/actions/workflows/ci.yml)
[![Security audit](https://github.com/RemiPelloux/Containust/actions/workflows/security.yml/badge.svg)](https://github.com/RemiPelloux/Containust/actions/workflows/security.yml)
[![License](https://img.shields.io/badge/license-MIT%20or%20Apache--2.0-blue.svg)](LICENSE-MIT)

## Why Containust

- **No daemon:** the CLI talks directly to the selected backend and persists state as files.
- **Declarative composition:** `.ctst` files describe components and `CONNECT` dependencies.
- **Local-first images:** `file://` directories and `tar://` archives work without a registry.
- **Rust SDK:** the parser, graph resolver, runtime types, and event APIs are reusable from Rust.
- **Platform-aware runtime:** native Linux isolation is selected on Linux; QEMU is used on macOS and Windows.

## Verified capabilities

| Area | Status | Notes |
| --- | --- | --- |
| `.ctst` lexer, parser, and validation | Working | Syntax, properties, imports, health checks, and invalid-input paths are covered. |
| Dependency graph and auto-wiring | Working | Topological ordering, cycle detection, and connection environment variables are covered. |
| Local image sources | Working | Existing `file://` directories and `tar://` archives can be resolved and extracted. |
| Image hashing and catalog | Working | SHA-256 validation and JSON catalog CRUD are covered. |
| State and logs | Working | JSON state and per-container logs round-trip on disk. |
| CLI parsing and Compose conversion | Working | `ctst` subcommands and the supported Compose subset have tests. |
| Linux isolation backend | Experimental | Requires Linux namespaces, a usable cgroups v2 hierarchy, mount permissions, and a valid rootfs. |
| QEMU backend | Experimental | Requires QEMU and network access for first-run Alpine assets; no cross-platform runtime test runs in this repository. |
| eBPF observability | Experimental | The API and feature-gated code compile; kernel attachment is not covered by the default test run. |

The default test run executes **410 tests**. **23 tests are intentionally ignored** because they require root privileges or a host cgroups/mount configuration; additional feature/platform-gated tests are listed only when their target is enabled.

## Platform requirements

| Host | Backend | Requirements |
| --- | --- | --- |
| Linux | Native | Linux 5.10+, user/mount/PID namespaces, cgroups v2, and mount permissions. |
| macOS | QEMU VM | QEMU 7+ (`brew install qemu`) and a Linux VM asset download on first use. |
| Windows | QEMU VM | QEMU 7+ and a Linux VM asset download on first use. |

Rust 1.86 or newer is required by the workspace manifest. The checked-in toolchain file selects the stable channel.

## Quick start

Build and verify the workspace:

```bash
git clone https://github.com/RemiPelloux/Containust.git
cd Containust
cargo check --workspace
cargo test --workspace --lib --tests
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Install the CLI locally:

```bash
cargo install --path crates/containust-cli
ctst --version
ctst --help
```

Inspect a composition without starting containers:

```bash
ctst plan examples/hello.ctst
ctst build examples/hello.ctst
```

Run a composition on a host with a supported backend:

```bash
ctst run examples/hello.ctst --detach
ctst ps --all
ctst logs hello
ctst stop hello
```

The Linux backend accepts local rootfs sources such as `file:///absolute/path` and `tar:///absolute/path/image.tar`. Registry-style image names are not pulled by `ctst build`; convert or prepare a local archive first.

## The `.ctst` format

```text
COMPONENT api {
    image = "file:///opt/images/api"
    command = ["/bin/api", "--listen", "0.0.0.0:8080"]
    port = 8080
    memory = "256MiB"
    env = { RUST_LOG = "info" }
}

COMPONENT db {
    image = "tar:///opt/images/postgres.tar"
    port = 5432
}

CONNECT api -> db
```

`IMPORT`, `COMPONENT`, `CONNECT`, environment maps, ports, volumes, health checks, and resource declarations are documented in [docs/CTST_LANG.md](docs/CTST_LANG.md). The parser rejects undefined connections and duplicate component names before deployment.

## CLI commands

| Command | Purpose |
| --- | --- |
| `ctst plan [FILE]` | Parse a composition and print deployment order. |
| `ctst build [FILE]` | Validate the composition and resolve declared image sources. |
| `ctst run [FILE]` | Create and start components through the selected backend. |
| `ctst ps [--all]` | List tracked containers. |
| `ctst exec CONTAINER COMMAND...` | Execute in a running container (Linux uses `nsenter`). |
| `ctst logs CONTAINER` | Read persisted logs. |
| `ctst stop [CONTAINER...]` | Stop named/identified containers, or all containers. |
| `ctst images` | List or remove catalog entries. |
| `ctst convert COMPOSE.yml` | Convert the supported Docker Compose subset to `.ctst`. |
| `ctst vm start/stop` | Manage the QEMU backend on non-Linux hosts. |

Run `ctst <command> --help` for command-specific options.

## Architecture

The workspace is split into nine crates with a downward dependency flow:

```text
containust-cli / containust-tui
          |
     containust-sdk
          |
containust-compose  containust-runtime  containust-image
          |                 |                 |
       containust-core   containust-ebpf       |
                    \     |                  /
                     containust-common
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for the dependency rules and backend design.

## Storage and security

- Immutable VM assets are cached under `~/.containust/cache/`.
- The runtime currently uses the resolved user data directory (`$HOME/.containust`, or `/var/lib/containust` when no writable home is available) for state, logs, rootfs data, and the image catalog.
- Read-only rootfs, capability dropping, cgroups, and namespace setup are implemented in the Linux path but must be validated on the target host.
- Remote HTTP(S) sources are recognized by the image layer; download policy, digest enforcement, and the global `--offline` flag are not yet complete.
- `--state-file` is accepted by the CLI parser but is not yet wired through every command.

## Development and audit

The repository uses `cargo test --workspace --lib --tests` for the deterministic default suite. Run the full `cargo test --workspace` command as well when your environment has a working `rustdoc`; on some macOS Rustup installations, the `rustdoc` shim can fail before doctests start.

Before opening a change, run:

```bash
cargo check --workspace
cargo test --workspace --lib --tests
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
```

Privileged namespace, mount, cgroup, and eBPF tests are marked `ignored` and should be run explicitly on a suitable Linux host. See [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) for coding standards and review expectations.

## Documentation

- [CLI reference](docs/CLI_REFERENCE.md)
- [.ctst language reference](docs/CTST_LANG.md)
- [SDK guide](docs/SDK_GUIDE.md)
- [Tutorials](docs/TUTORIALS.md)
- [Error reference](docs/ERRORS.md)
- [Docker migration](docs/MIGRATION_FROM_DOCKER.md)
- [Architecture](ARCHITECTURE.md)
- [Technical specification](docs/SPEC.md)

## License

Containust is available under either the [MIT License](LICENSE-MIT) or the [Apache License 2.0](LICENSE-APACHE), at your option.
