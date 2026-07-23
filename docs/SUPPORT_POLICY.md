# Support Policy

Binding since `1.0.0`. Updated for Sprint 10 (`1.1.0`).

## Supported platforms

| OS | Arch | Runtime backend | Notes |
|---|---|---|---|
| Linux | x86_64, aarch64 | Native (namespaces, cgroups v2, OverlayFS) | Kernel 5.10+ recommended |
| macOS | x86_64, aarch64 | QEMU VM + agent | QEMU 7+; HVF when available |
| Windows | x86_64 | QEMU VM + agent | QEMU 7+; WHPX when available |

## Compatibility guarantees

See [`VERSIONING.md`](VERSIONING.md). Summary:

- **SDK** (`containust-sdk` + curated `containust-common`): SemVer from `1.0.0`.
- **`.ctst`**: additive keywords are MINOR; breaking syntax is MAJOR.
- **`state.json`**: older schemas migrate; newer schemas fail closed.

## Port publishing (`ports` / `EXPOSE`)

Ports are published with identity mapping only (host port == container port).
`EXPOSE host:container` with differing ports fails closed at deploy.

| Platform | Publish path | Capability needs |
|---|---|---|
| Linux | Component with published ports shares the host network namespace (like `docker run --network host`); the process binds host ports directly | Root (or `CAP_NET_BIND_SERVICE` for ports < 1024) |
| macOS / Windows | QEMU user-net `hostfwd` rules bound at VM boot; adding ports to a live VM requires `ctst vm stop` + redeploy | None (userspace) |

veth/NAT-based publishing with host/container remapping on Linux is deferred
(tracked for a later sprint). Components without published ports keep an
isolated network namespace on Linux.

## Issue severity

| Severity | Meaning | Response target (best effort) |
|---|---|---|
| P0 | Security escape, data loss, unrecoverable corruption | Immediate hotfix / advisory |
| P1 | Wrong lifecycle result, false offline allow, crash on supported path | Next patch |
| P2 | Degraded UX, missing docs, non-blocking CI flake | Next minor |
| P3 | Nice-to-have, backlog features | Backlog |

## Deprecation

- Announce in `CHANGELOG.md` and docs at least one MINOR before removal.
- Removals happen only in MAJOR releases after `1.0.0`.

## Explicitly deferred (not supported unless listed above)

- `EXPOSE` host/container port remapping on Linux (veth/NAT publish path)
- Apple notarization / Windows Authenticode (cosign keyless signs `SHA256SUMS`; see `PACKAGING.md`)
- PID / user-namespace wiring on the Linux spawn path
- Multi-network mesh, DNS / service discovery
- Rolling updates / declarative `plan` apply diffs
- Volume drivers, snapshots, encrypted local storage
- Remote execution / orchestration

Shipped since GA (removed from this list): OCI registry pulls with auth
(`ctst pull`, `1.1.0`), `ports` / `restart` / `healthcheck` enforcement
(`1.1.0`), GitHub Release binaries + `.deb`/`.rpm` + in-tree Homebrew +
winget template + cosign-signed checksums (`1.1.0`).
