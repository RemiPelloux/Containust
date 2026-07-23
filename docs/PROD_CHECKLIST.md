# Production Readiness Checklist (Sprint 10 / `1.1.0`)

Every item must be checked (or explicitly deferred with an owner) before the
`v1.1.0` tag. IDs map to `roadmap.md` Sprint 10.

## Wave 1 — Foundation

- [x] **P10.1** Trackers (`work.md`, `roadmap.md`) match HEAD version and sprint state.
- [ ] **P10.2** Privileged Linux CI job runs the `#[ignore]` core/CLI fixtures as root
      (namespaces, cgroups v2, mounts, capabilities, sprint3 offline gate).
      _Job added; fixture fixes (cgroup rmdir, forked user-ns probes) awaiting green run._
- [x] **P10.3** Linux port publish path documented in `CLI_REFERENCE.md` and
      `SUPPORT_POLICY.md` (landed with Wave 3).

## Wave 2 — OCI image pull

- [x] **P10.4** `oci://` scheme resolves registry manifests (index → platform
      manifest → layer blobs) for Docker Hub and GHCR.
- [x] **P10.5** Digest pin required by default; `--offline` rejects registry
      references before any connection is opened.
- [x] **P10.6** Auth via `CONTAINUST_REGISTRY_TOKEN` or `~/.docker/config.json`;
      credentials never logged and never written to `state.json`.
- [x] **P10.7** Pulled layers land in the content-addressed store and catalog as
      `image://name@sha256:...`.
- [x] **P10.8** `ctst pull` CLI command; preset hints updated; docs/examples agree.

## Wave 3 — Runtime features

- [x] **P10.9** `ports` list and `EXPOSE` enforced with identity mapping (Linux
      host-network publish + VM multi-hostfwd); host/container remapping fails
      closed; singular `port` still feeds CONNECT env injection.
- [x] **P10.10** `restart = "never" | "on-failure" | "always"` enforced by the
      state machine and reconciliation (daemonless, on every `ps`/`run` pass).
- [x] **P10.11** `healthcheck` blocks execute on interval and drive restart policy.
- [x] **P10.12** State schema bumped to 3 (`ports`, `restart`, `healthcheck`,
      `health`, `restart_count`); legacy states migrate via serde defaults.
- [x] **P10.13** All bundled examples parse and validate
      (`all_bundled_examples_parse_and_validate`); healthcheck example deploys
      end-to-end against the fake backend.

## Wave 4 — Packaging and release

- [x] **P10.14** In-tree Homebrew formula (`Formula/ctst.rb`) with install docs
      in `PACKAGING.md`; dedicated tap + automated sha bump tracked as follow-up.
- [x] **P10.15** `.deb` / `.rpm` built by the `linux-packages` release job via
      nfpm (`packaging/nfpm.yaml`).
- [x] **P10.16** winget manifest template in `packaging/winget/` with
      submission instructions.
- [x] **P10.17** Release verification runbook in `RUNBOOKS.md`; `SHA256SUMS`
      signed keylessly with cosign in the release workflow.
- [ ] **P10.18** `CHANGELOG.md` updated; `SUPPORT_POLICY.md` deferred list pruned;
      `v1.1.0` tagged with green CI. _Awaiting green CI on main before tagging._

## Verification gates (all waves)

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace`
- [x] `cargo deny check`
- [ ] CI matrix green: Linux, macOS, Windows, QEMU smoke, privileged Linux job.
