# Support Policy

Binding since `1.0.0`. Updated for Sprint 11 (`1.2.0`).

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

`EXPOSE` supports identity (`EXPOSE 8080`) and remapping (`EXPOSE 80:8080`).

| Platform | Publish path | Capability needs |
|---|---|---|
| Linux (identity, no explicit `network`) | Host network namespace; process binds host ports directly | Root (or `CAP_NET_BIND_SERVICE` for ports < 1024) |
| Linux (remap or named/`bridge` network) | Private/shared netns + userspace TCP forwarder (`127.0.0.1:host` → container port) | Root for userns/netns; no `CAP_NET_ADMIN` |
| macOS / Windows | QEMU user-net `hostfwd` (host→guest remap supported); adding ports to a live VM requires `ctst vm stop` + redeploy | None (userspace) |

Named networks (`network = "bridge"` or a custom name) share one netns per
project network. Peer names are written into `/etc/hosts` as `127.0.0.1` so
`CONNECT` `_HOST` variables resolve on the shared loopback. Shared networks
disable per-container user namespaces (the persisted netns is owned by the
init userns); PID namespaces remain on. Unspecified `network` uses a private
netns with full user+PID isolation.

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

## Linux spawn isolation (user + PID namespaces)

Linux containers enable user + PID namespaces by default (pipe-synced uid/gid
maps and post-`CLONE_NEWPID` double-fork so container init is PID 1). Root or
delegated user namespaces are recommended.

## Explicitly deferred (not supported unless listed above)

- Apple notarization / Windows Authenticode (cosign keyless signs `SHA256SUMS`; see `PACKAGING.md`)
- Full DNS / multi-network mesh (named shared-netns + `/etc/hosts` foundations ship in Sprint 11)
- Rolling updates / declarative `plan` apply diffs
- Volume drivers, snapshots, encrypted local storage
- Remote execution / orchestration

Shipped since GA (removed from this list): OCI registry pulls with auth
(`ctst pull`, `1.1.0`), `ports` / `restart` / `healthcheck` enforcement
(`1.1.0`), GitHub Release binaries + `.deb`/`.rpm` + in-tree Homebrew +
winget template + cosign-signed checksums (`1.1.0`), Linux user+PID namespace
default-on spawn, `EXPOSE` remapping, named networks (`1.2.0` track).
