# Containust

Containust is a daemon-less container runtime and declarative composition tool written in Rust. It is designed for local, sovereign, and air-gapped workflows where a long-running privileged daemon is undesirable.

> **Project status: alpha (`0.8.0`).** Sprint 7 complete: versioning contract, release metadata/checksums, packaging deferrals, CI docs+coverage, perf budgets, runbooks. Remaining toward 1.0: beta freeze (Sprint 8), GA (Sprint 9).

[![CI](https://github.com/RemiPelloux/Containust/actions/workflows/ci.yml/badge.svg)](https://github.com/RemiPelloux/Containust/actions/workflows/ci.yml)
[![Security audit](https://github.com/RemiPelloux/Containust/actions/workflows/security.yml/badge.svg)](https://github.com/RemiPelloux/Containust/actions/workflows/security.yml)
[![License](https://img.shields.io/badge/license-MIT%20or%20Apache--2.0-blue.svg)](LICENSE-MIT)

## Why Containust

- **No daemon:** the CLI talks directly to the selected backend and persists state as files.
- **Declarative composition:** `.ctst` files describe components and `CONNECT` dependencies.
- **Local-first images:** `file://` directories, `tar://` archives, and curated `preset://alpine` / `preset://busybox` downloads (~4&nbsp;MiB official rootfs) work without Docker Hub.
- **Rust SDK:** the parser, graph resolver, runtime types, and event APIs are reusable from Rust.
- **Platform-aware runtime:** native Linux isolation is selected on Linux; QEMU is used on macOS and Windows.

## Verified capabilities

| Area | Status | Notes |
| --- | --- | --- |
| `.ctst` lexer, parser, and validation | Working | Syntax, properties, imports, health checks, and invalid-input paths are covered. |
| Dependency graph and auto-wiring | Working | Topological ordering, cycle detection, and connection environment variables are covered. |
| Local image sources | Working | Existing `file://` directories and `tar://` archives can be resolved and extracted. |
| Content-addressed image import | Working | `ctst build` deterministically imports directories/archives into `layers/<sha256>/`, records supply-chain metadata, and supports `--dry-run`. |
| Curated presets | Working | `preset://alpine` / `preset://busybox` (and version pins like `alpine:3.21`) download official minirootfs archives with pinned SHA-256; `ctst images --presets` lists them. Node/PHP/etc. need future OCI Hub pull. |
| Offline / air-gapped execution | Working | Imported images run from `image://name@sha256:<digest>` with `--offline`; copying `images/` + `layers/` between machines is sufficient. Remote fetch is opt-in, digest-pinned, size-capped, and retried. |
| Image hashing and catalog | Working | SHA-256 validation; the JSON catalog is lock-guarded, atomically written, deduplicated, and layer-validated. |
| State and logs | Working | Schema-versioned JSON state uses atomic writes and cross-process locks; per-container logs persist until removal. |
| Project isolation and reconciliation | Working | Each composition uses project-local state/data; `ctst ps` repairs stale processes and removes project-owned orphan runtime resources. |
| CLI parsing and Compose conversion | Working | `ctst` subcommands and the supported Compose subset have tests. |
| Linux isolation backend | Experimental | Linux compilation and non-privileged tests pass at the Rust 1.88 MSRV. Full release validation still requires a delegated cgroups v2 hierarchy, user namespaces, mount permissions, and a valid rootfs. |
| QEMU backend | Experimental | Requires QEMU and network access for first-run Alpine assets; no cross-platform runtime test runs in this repository. |
| eBPF observability | Experimental | The API and feature-gated code compile; kernel attachment is not covered by the default test run. |

The default macOS test run passes **470 tests** with **23 tests intentionally ignored** because they require root privileges or a host cgroups/mount configuration; the Linux (Rust 1.88) run passes **480 tests** with 26 ignored. Additional feature/platform-gated tests are listed only when their target is enabled. The suite includes a 2,000-component graph regression test to protect planning performance, plus thread and subprocess contention tests for state durability.

## Platform requirements

| Host | Backend | Requirements |
| --- | --- | --- |
| Linux | Native | Linux 5.10+, user/mount/PID namespaces, cgroups v2, and mount permissions. |
| macOS | QEMU VM | QEMU 7+ (`brew install qemu`) and a Linux VM asset download on first use. |
| Windows | QEMU VM | QEMU 7+ and a Linux VM asset download on first use. |

Rust 1.88 or newer is required by the workspace manifest. The checked-in toolchain file selects the stable channel.

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
ctst rm hello
```

The Linux backend accepts local rootfs sources such as `file:///absolute/path` and `tar:///absolute/path/image.tar`. Registry-style image names are not pulled by `ctst build`; convert or prepare a local archive first. Use `--offline` (or `CONTAINUST_OFFLINE=1`) to reject remote image/import sources before backend or network access, and `--state-file /path/state.json` (or `CONTAINUST_STATE_FILE`) to select an isolated state, log, image, and runtime-data root.

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
| `ctst rm [--force] CONTAINER...` | Remove stopped containers and project-owned rootfs, logs, cgroups, and state entries. |
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
- Runtime data is project-scoped under `.containust/` next to the selected composition. State is stored in `.containust/state/state.json`; logs, rootfs data, and the image catalog remain isolated within the same project directory.
- Read-only rootfs, capability dropping, cgroups, and namespace setup are implemented in the Linux path but must be validated on the target host.
- Remote HTTP(S) sources are recognized by the image layer, but authenticated downloads, digest policy, and resumable caching are planned for Sprint 3.
- `--offline` and `--state-file` are wired through all stateful CLI commands; `CONTAINUST_STATE_FILE` provides the environment override and invalid or inaccessible paths fail explicitly.
- `ctst stop` retains rootfs and logs for inspection while clearing the running process/cgroup; `ctst rm` performs permanent project-owned cleanup. Host volume source data is never deleted.
- Port forwarding and network isolation are intentionally deferred to the cross-platform networking milestone.

## Development and audit

The repository uses `cargo test --workspace --lib --tests` for the deterministic default suite. Run the full `cargo test --workspace` command as well when your environment has a working `rustdoc`; on some macOS Rustup installations, the `rustdoc` shim can fail before doctests start.

Before opening a change, run:

```bash
cargo check --workspace
cargo test --workspace --lib --tests
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
cargo audit
```

`cargo audit` reports no known vulnerability or unsoundness advisories for the locked dependency graph. `cargo deny check` passes the advisory, dependency, license, and source policies. Keep the lockfile committed and rerun both checks when dependencies change.

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
