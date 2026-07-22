# Containust Versioning and Compatibility

This document is the compatibility contract for crate releases, `.ctst` files,
`state.json`, the SDK, and related on-wire versions. Product SemVer lives in the
workspace; schema and protocol versions are independent integers.

## Workspace crate version

**Source of truth:** root [`Cargo.toml`](../Cargo.toml)

1. `[workspace.package] version` — inherited by every crate via `version.workspace = true`.
2. `[workspace.dependencies]` path crates — each internal crate pin must match the
   same SemVer string (nine `containust-*` entries).

Bump checklist:

```bash
# Prefer cargo-edit when available:
cargo set-version --workspace <NEW>

# Then verify workspace.dependencies pins still match [workspace.package].version
# Update CHANGELOG.md ([Unreleased] → [NEW])
# Align doc banners that hardcode the version (CLI_REFERENCE, SDK_GUIDE, README)
```

Until `1.0.0`, the public API may change in MINOR releases; prefer the SDK
(`containust-sdk` + curated `containust-common` types) over engine crates.

| Change | SemVer impact |
|---|---|
| Breaking SDK API or `.ctst` syntax | MAJOR |
| Backward-compatible features / CLI | MINOR |
| Fixes, docs, performance | PATCH |

See also [CONTRIBUTING.md](CONTRIBUTING.md#release-process).

## `state.json` schema

| Constant | Value | Defined in |
|---|---|---|
| `STATE_SCHEMA_VERSION` / `CURRENT_STATE_SCHEMA` | `2` | `containust-common` / `containust-runtime` |

- **Older** schemas migrate forward on load.
- **Newer** schemas are rejected (fail closed).
- State files must never store secrets (see security rules).

## `.ctst` compositions

- Composition files have **no in-file schema version** today; the parser always
  expects the current language surface.
- Additive keywords are MINOR; removing or changing meaning of existing syntax
  is MAJOR.
- Language documentation version in `CTST_LANG.md` describes the language doc
  revision, not the crate SemVer.

## SDK compatibility

- Crate version of `containust-sdk` equals the workspace version.
- Supported public surface: `containust-sdk` and the types it re-exports from
  `containust-common`.
- Engine crates (`runtime`, `image`, `compose`, …) are not a stability promise
  for external consumers.

## Related versioned surfaces

| Surface | Constant | Notes |
|---|---|---|
| VM agent RPC | `PROTOCOL_VERSION = 1` | Mismatch fails closed |
| Image catalog | `tool_version` string | Records creating crate version |
| CLI binary | `CARGO_PKG_VERSION` | Shown by `ctst --version` |

## Release artifacts

Tagged releases (`v*`) build multi-platform binaries with SHA-256 checksums via
[`.github/workflows/release.yml`](../.github/workflows/release.yml). Signing and
packaging channels are tracked under Sprint 7 (L7.2 / L7.3).
