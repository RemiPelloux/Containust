# Support Policy (pre-GA draft)

Draft for Sprint 9 / G9.3. Binding at `1.0.0`.

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

## Explicitly deferred (not supported at GA unless listed above)

- OCI registry auth / Hub pulls for arbitrary names
- Homebrew / deb / RPM / winget packages (see `PACKAGING.md`)
- Code signing / notarization
