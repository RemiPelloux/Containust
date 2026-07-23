# Containust

Containust is a daemon-less container runtime and declarative composition tool written in Rust. It is designed for local, sovereign, and air-gapped workflows where a long-running privileged daemon is undesirable.

> **Project status: GA (`1.1.0`).** Installable release packages, OCI pulls (`ctst pull`), and runtime enforcement of `ports` / `restart` / `healthcheck`. See `docs/SUPPORT_POLICY.md` and `docs/PROD_CHECKLIST.md`.

[![CI](https://github.com/RemiPelloux/Containust/actions/workflows/ci.yml/badge.svg)](https://github.com/RemiPelloux/Containust/actions/workflows/ci.yml)
[![Security audit](https://github.com/RemiPelloux/Containust/actions/workflows/security.yml/badge.svg)](https://github.com/RemiPelloux/Containust/actions/workflows/security.yml)
[![License](https://img.shields.io/badge/license-MIT%20or%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Release](https://img.shields.io/github/v/release/RemiPelloux/Containust)](https://github.com/RemiPelloux/Containust/releases/latest)

## Why Containust

- **No daemon:** the CLI talks directly to the selected backend and persists state as files.
- **Declarative composition:** `.ctst` files describe components and `CONNECT` dependencies.
- **Local-first images:** `file://`, `tar://`, curated `preset://alpine` / `preset://busybox`, and digest-pinned `oci://` / `ctst pull` from Docker Hub and GHCR.
- **Runtime policies:** `ports` / `EXPOSE`, restart policies, and healthchecks are enforced (not just parsed).
- **Rust SDK:** the parser, graph resolver, runtime types, and event APIs are reusable from Rust.
- **Platform-aware runtime:** native Linux isolation on Linux; QEMU on macOS and Windows.

## Verified capabilities

| Area | Status | Notes |
| --- | --- | --- |
| `.ctst` lexer, parser, and validation | Working | Syntax, properties, imports, `EXPOSE`, health checks, and invalid-input paths are covered. |
| Dependency graph and auto-wiring | Working | Topological ordering, cycle detection, and connection environment variables are covered. |
| Local image sources | Working | `file://` directories and `tar://` archives resolve and extract safely. |
| OCI registry pull | Working | `ctst pull` / `oci://` pulls Docker Hub and GHCR images with digest verification and optional auth. |
| Content-addressed image import | Working | `ctst build` imports into `layers/<sha256>/`, records supply-chain metadata, and supports `--dry-run`. |
| Curated presets | Working | `preset://alpine` / `preset://busybox` download pinned Alpine minirootfs; other Hub names use `ctst pull`. |
| Offline / air-gapped execution | Working | Run from `image://name@sha256:<digest>` with `--offline`; copy `images/` + `layers/` between machines. |
| Ports, restart, healthcheck | Working | Identity port publish (Linux host-net / VM `hostfwd`); daemonless restart + health probes on reconcile. |
| Image hashing and catalog | Working | SHA-256 validation; lock-guarded atomic catalog with layer validation. |
| State and logs | Working | Schema v3 JSON state with atomic writes and locks; detached containers log to per-container files. |
| Project isolation and reconciliation | Working | Project-local state; `ctst ps` repairs stale processes and cleans project-owned orphans. |
| CLI parsing and Compose conversion | Working | Subcommands and the supported Compose subset have tests. |
| Linux isolation backend | Working | Privileged CI exercises namespaces, cgroups v2, mounts, and the offline gate as root. |
| QEMU backend | Working | CI QEMU smoke on macOS; first-run Alpine VM assets cached under `~/.containust/cache/`. |
| eBPF observability | Experimental | Feature-gated API compiles; kernel attachment is not in the default test run. |
| Packaging | Working | GitHub Release tarballs/zips, `.deb`/`.rpm`, in-tree Homebrew formula, winget template, cosign-signed `SHA256SUMS`. |

The default workspace suite passes **607 tests** with **20 tests intentionally ignored** (privileged fixtures that also run in the `privileged-linux` CI job). The suite includes a 2,000-component graph regression test and contention tests for state durability.

## Platform requirements

| Host | Backend | Requirements |
| --- | --- | --- |
| Linux | Native | Linux 5.10+, user/mount/PID namespaces, cgroups v2, and mount permissions. |
| macOS | QEMU VM | QEMU 7+ (`brew install qemu`) and a Linux VM asset download on first use. |
| Windows | QEMU VM | QEMU 7+ and a Linux VM asset download on first use. |

Rust 1.88 or newer is required by the workspace manifest. The checked-in toolchain file selects the stable channel.

## Install

New here? Start with **[docs/HowToUse.md](docs/HowToUse.md)**.

Prefer a verified release binary (see [docs/PACKAGING.md](docs/PACKAGING.md) and [docs/RUNBOOKS.md](docs/RUNBOOKS.md)):

```bash
# Example: Linux x86_64 tarball
VERSION=1.1.0
curl -LO "https://github.com/RemiPelloux/Containust/releases/download/v${VERSION}/ctst-x86_64-unknown-linux-gnu.tar.gz"
curl -LO "https://github.com/RemiPelloux/Containust/releases/download/v${VERSION}/ctst-x86_64-unknown-linux-gnu.tar.gz.sha256"
sha256sum -c "ctst-x86_64-unknown-linux-gnu.tar.gz.sha256"
tar xzf "ctst-x86_64-unknown-linux-gnu.tar.gz"
sudo install -m 755 ctst /usr/local/bin/ctst
ctst --version
```

Also available: `.deb` / `.rpm` assets, in-tree Homebrew (`brew install --formula ./Formula/ctst.rb`), and a winget manifest template. Or build from source:

```bash
git clone https://github.com/RemiPelloux/Containust.git
cd Containust
cargo install --path crates/containust-cli
```

## Quick start

Pull an OCI image (digest-pinned into the local catalog):

```bash
ctst pull alpine:3.21
# → image://library/alpine@sha256:...
```

Inspect and run a composition:

```bash
ctst plan examples/hello.ctst
ctst build examples/hello.ctst
ctst run examples/hello.ctst --detach
ctst ps --all
ctst logs hello
ctst stop hello
ctst rm hello
```

Image sources: `file://`, `tar://`, `preset://`, `oci://` / bare `name:tag` via `ctst pull`, and pinned `image://name@sha256:...`. Use `--offline` (or `CONTAINUST_OFFLINE=1`) to reject remote sources before any network access. Use `--state-file` (or `CONTAINUST_STATE_FILE`) for an isolated state/log/image root.

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
| `ctst pull IMAGE` | Pull an OCI image (Docker Hub / GHCR) into the local catalog. |
| `ctst run [FILE]` | Create and start components; reconcile restart/healthcheck policies. |
| `ctst ps [--all]` | List tracked containers (also runs reconciliation). |
| `ctst exec CONTAINER COMMAND...` | Execute in a running container (Linux uses `nsenter`). |
| `ctst logs CONTAINER` | Read persisted logs. |
| `ctst stop [CONTAINER...]` | Stop named/identified containers, or all containers. |
| `ctst rm [--force] CONTAINER...` | Remove stopped containers and project-owned rootfs, logs, cgroups, and state entries. |
| `ctst images` | List or remove catalog entries; `--presets` lists curated presets. |
| `ctst convert COMPOSE.yml` | Convert the supported Docker Compose subset to `.ctst`. |
| `ctst vm start/stop` | Manage the QEMU backend on non-Linux hosts. |

Run `ctst <command> --help` for command-specific options. Full reference: [docs/CLI_REFERENCE.md](docs/CLI_REFERENCE.md).

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
- Runtime data is project-scoped under `.containust/` next to the composition. State is `.containust/state/state.json` (schema v3); logs, rootfs, and the image catalog stay in the same project tree.
- Linux path: read-only rootfs by default, capability drop, cgroups v2, namespaces — exercised in privileged CI.
- Remote OCI/HTTPS fetches are digest-pinned, size/timeout capped, and rejected under `--offline`. Registry auth uses env vars or `~/.docker/config.json`; credentials are never logged or written to `state.json`.
- Published ports use identity mapping (host port == container port). Linux components with ports share the host network; remapping fails closed (veth/NAT is a later milestone).
- `ctst stop` retains rootfs and logs; `ctst rm` cleans project-owned resources. Host volume source data is never deleted.

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

- **[How to use](docs/HowToUse.md)** — install, first pull/run, everyday commands, ports & offline
- [CLI reference](docs/CLI_REFERENCE.md)
- [.ctst language reference](docs/CTST_LANG.md)
- [Tutorials](docs/TUTORIALS.md)
- [SDK guide](docs/SDK_GUIDE.md)
- [Packaging & install](docs/PACKAGING.md)
- [Support policy](docs/SUPPORT_POLICY.md)
- [Operator runbooks](docs/RUNBOOKS.md)
- [Docker migration](docs/MIGRATION_FROM_DOCKER.md)
- [Architecture](ARCHITECTURE.md)
- [Roadmap](roadmap.md)

## License

Containust is available under either the [MIT License](LICENSE-MIT) or the [Apache License 2.0](LICENSE-APACHE), at your option.
