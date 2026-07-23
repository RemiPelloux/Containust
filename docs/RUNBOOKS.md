# Operator Runbooks

Procedures for upgrade, rollback, incidents, cache recovery, and cleanup.
Companion to [`VERSIONING.md`](VERSIONING.md) and [`PACKAGING.md`](PACKAGING.md).

## Verify a release download

Mandatory before installing any release artifact (P10.17).

```bash
VERSION="X.Y.Z"
TARGET="x86_64-unknown-linux-gnu"   # your platform triple
BASE="https://github.com/RemiPelloux/Containust/releases/download/v${VERSION}"

# 1. Download the artifact and the signed checksum manifest.
curl -LO "${BASE}/ctst-${TARGET}.tar.gz"
curl -LO "${BASE}/SHA256SUMS"
curl -LO "${BASE}/SHA256SUMS.sigstore.json"

# 2. Verify the manifest signature (cosign keyless / Sigstore).
cosign verify-blob SHA256SUMS \
  --bundle SHA256SUMS.sigstore.json \
  --certificate-identity-regexp 'https://github.com/RemiPelloux/Containust/\.github/workflows/release\.yml.*' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com

# 3. Verify the artifact checksum against the signed manifest.
grep "ctst-${TARGET}.tar.gz" SHA256SUMS | sha256sum -c -
```

Abort the install on any verification failure — do not fall back to an
unverified binary. If `cosign` is unavailable, the per-artifact `.sha256`
files still provide integrity (but not provenance) checking.

## Upgrade

1. Note current version: `ctst --version` (includes `git=` / `built=` on release builds).
2. Stop running projects: `ctst stop` (or stop named containers).
3. **Backup** project state: `cp .containust/state.json .containust/state.json.bak`.
4. Install the new binary (release tarball + SHA-256, or rebuild from tag).
5. Run `ctst doctor` and confirm backend, cache, and (on Linux) cgroup readiness.
6. Redeploy: `ctst run compose.ctst` (or project-specific command).
7. Confirm: `ctst ps` and application health checks.

State schema migrates forward automatically when `schema_version` is older than
`STATE_SCHEMA_VERSION`. Newer schemas fail closed — do not downgrade across a
schema bump without restoring a backup.

### Rehearsal checklist (B8.3)

Automated coverage: `cargo test -p containust-runtime --test upgrade_rehearsal`.

Manual dry-run before a beta/GA cut:

1. Create a throwaway project with one container entry, a log line, and a catalog image.
2. Upgrade binary; confirm `state.json` schema migrates and logs/catalog remain.
3. Drop a partial `.state.json.*.tmp` beside state; confirm load still returns the last good file.
4. Empty `state.json` on purpose; restore from `.bak`; confirm containers return while logs/catalog stay.

## Rollback

1. Stop containers with the newer binary if it is still functional.
2. Restore the previous `ctst` binary (keep prior release artifacts).
3. Restore `.containust/state.json` from backup if the upgrade wrote incompatible data.
4. Restore image catalog / layer store only if import changed digests unexpectedly.
5. `ctst doctor` → `ctst ps` → redeploy.

## Incident: deploy or stop failure

1. Capture the CLI `error[CODE]` line and hint.
2. `ctst doctor` for host readiness.
3. Inspect logs: `ctst logs <name-or-id>`.
4. On VM backends: `ctst vm stop` then `ctst vm start` if the agent is wedged.
5. If state looks torn, do **not** hand-edit secrets into `state.json`; restore from backup.

## Cache recovery (VM assets)

Offline / corrupt asset failures fail closed.

```bash
# Default global cache
rm -rf ~/.containust/cache/vm
# Retry online once to re-fetch pinned digests
ctst vm start
```

Project-local data lives beside the `.ctst` file under `.containust/`.

## Data cleanup

```bash
ctst stop
ctst remove --force <name-or-id>   # per container
# Or remove project workspace carefully:
# rm -rf .containust/   # destroys project state, rootfs, logs for that project
```

Shared immutable VM assets under `~/.containust/cache/` can be deleted to reclaim
disk; the next `vm start` re-downloads when network is allowed.
