# Containust

**Daemon-less containers for local, sovereign, and air-gapped work.**

Compose stacks in `.ctst`, pull digest-pinned images, run without a privileged
daemon. Native Linux isolation; QEMU on macOS and Windows.

[![CI](https://github.com/RemiPelloux/Containust/actions/workflows/ci.yml/badge.svg)](https://github.com/RemiPelloux/Containust/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/RemiPelloux/Containust)](https://github.com/RemiPelloux/Containust/releases/latest)
[![License](https://img.shields.io/badge/license-Commercial-blue.svg)](LICENSE)

**GA `1.1.0`** · [Landing page](site/index.html) · [How to use](docs/HowToUse.md) · [Docs map](docs/README.md)

---

## Why

- **No daemon** — each `ctst` command talks to the backend and writes file state
- **Declarative `.ctst`** — `COMPONENT`, `CONNECT`, `EXPOSE`, restart, healthcheck
- **Local-first images** — `file://`, `tar://`, `preset://`, digest-pinned `ctst pull`
- **Air-gapped ready** — `--offline` rejects the network before connect
- **Rust SDK** — parser, graph, and runtime APIs without the CLI

## Install

```bash
VERSION=1.1.0
TARGET=x86_64-unknown-linux-gnu   # or aarch64-*-gnu, *-apple-darwin, …
curl -LO "https://github.com/RemiPelloux/Containust/releases/download/v${VERSION}/ctst-${TARGET}.tar.gz"
curl -LO "https://github.com/RemiPelloux/Containust/releases/download/v${VERSION}/ctst-${TARGET}.tar.gz.sha256"
sha256sum -c "ctst-${TARGET}.tar.gz.sha256"
tar xzf "ctst-${TARGET}.tar.gz" && sudo install -m 755 ctst /usr/local/bin/ctst
```

Also: `.deb` / `.rpm`, [Homebrew formula](Formula/ctst.rb), or
`cargo install --path crates/containust-cli`. Details in [docs/PACKAGING.md](docs/PACKAGING.md).

macOS / Windows need **QEMU 7+**.

## Quick start

```bash
ctst pull alpine:3.21
ctst plan examples/alpine_preset.ctst
ctst build examples/alpine_preset.ctst
ctst run examples/alpine_preset.ctst --detach
ctst ps --all
ctst logs app
ctst stop app && ctst rm app
```

```text
COMPONENT api {
    image   = "file:///opt/images/api"
    port    = 8080
    memory  = "256MiB"
}

COMPONENT db {
    image = "tar:///opt/images/postgres.tar"
    port  = 5432
}

CONNECT api -> db
EXPOSE 8080
```

## Commands

| Command | Purpose |
| --- | --- |
| `ctst plan` / `build` / `run` | Validate, import images, start |
| `ctst pull` | OCI pull into the local catalog |
| `ctst ps` / `logs` / `exec` | Inspect |
| `ctst stop` / `rm` | Tear down |
| `ctst convert` | Compose → `.ctst` |
| `ctst vm start/stop` | QEMU lifecycle (macOS / Windows) |

Full reference: [docs/CLI_REFERENCE.md](docs/CLI_REFERENCE.md).

## Platforms

| Host | Backend | Needs |
| --- | --- | --- |
| Linux 5.10+ | Native namespaces + cgroups v2 | Root or delegated userns |
| macOS / Windows | QEMU + agent | QEMU 7+, first-run VM assets |

## Docs

| Path | Role |
| --- | --- |
| [site/](site/) | Product landing page |
| [docs/HowToUse.md](docs/HowToUse.md) | Day-to-day operator guide |
| [docs/README.md](docs/README.md) | Full documentation map |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Crate layers |
| [roadmap.md](roadmap.md) | Sprint roadmap |

## Develop

```bash
cargo test --workspace --lib --tests
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
```

See [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md).

## License

Containust is proprietary and source-available under the
[Containust Commercial License](LICENSE). Evaluation use is allowed;
production, redistribution, and commercial embedding require a paid license.
See [COMMERCIAL.md](COMMERCIAL.md).
