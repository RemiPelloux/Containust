# Production Readiness Checklist (Sprint 10 / `1.1.0`)

Every item must be checked (or explicitly deferred with an owner) before the
`v1.1.0` tag. IDs map to `roadmap.md` Sprint 10.

## Wave 1 — Foundation

- [ ] **P10.1** Trackers (`work.md`, `roadmap.md`) match HEAD version and sprint state.
- [ ] **P10.2** Privileged Linux CI job runs the `#[ignore]` core/CLI fixtures as root
      (namespaces, cgroups v2, mounts, capabilities, sprint3 offline gate).
- [ ] **P10.3** Linux port publish path documented in `CLI_REFERENCE.md` and
      `SUPPORT_POLICY.md` (lands with Wave 3).

## Wave 2 — OCI image pull

- [ ] **P10.4** `oci://` scheme resolves registry manifests (index → platform
      manifest → layer blobs) for Docker Hub and GHCR.
- [ ] **P10.5** Digest pin required by default; `--offline` rejects registry
      references before any connection is opened.
- [ ] **P10.6** Auth via `CONTAINUST_REGISTRY_TOKEN` or `~/.docker/config.json`;
      credentials never logged and never written to `state.json`.
- [ ] **P10.7** Pulled layers land in the content-addressed store and catalog as
      `image://name@sha256:...`.
- [ ] **P10.8** `ctst pull` CLI command; preset hints updated; docs/examples agree.

## Wave 3 — Runtime features

- [ ] **P10.9** `ports = ["host:container"]` enforced (Linux publish + VM multi-hostfwd);
      singular `port` still feeds CONNECT env injection.
- [ ] **P10.10** `restart = "never" | "on-failure" | "always"` enforced by the
      state machine and reconciliation.
- [ ] **P10.11** `healthcheck` blocks execute on interval and drive restart policy.
- [ ] **P10.12** State schema migration covers any new persisted fields.
- [ ] **P10.13** Example compositions (`examples/healthcheck_example.ctst`,
      nginx/redis templates) deploy without "unsupported property" errors.

## Wave 4 — Packaging and release

- [ ] **P10.14** Homebrew formula published or stubbed with install docs.
- [ ] **P10.15** `.deb` / `.rpm` artifacts produced by the release workflow (nfpm).
- [ ] **P10.16** winget manifest documented for the Windows zip.
- [ ] **P10.17** SHA-256 verification script in `RUNBOOKS.md`; cosign signing
      enabled or deferred with an owner.
- [ ] **P10.18** `CHANGELOG.md` updated; `SUPPORT_POLICY.md` deferred list pruned;
      `v1.1.0` tagged with green CI.

## Verification gates (all waves)

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `cargo deny check`
- [ ] CI matrix green: Linux, macOS, Windows, QEMU smoke, privileged Linux job.
