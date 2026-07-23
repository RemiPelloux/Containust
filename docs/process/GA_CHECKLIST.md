# `1.0.0` GA Checklist (Sprint 9)

## G9.1 Security sign-off

| Check | Evidence |
|---|---|
| Threat model current | [`THREAT_MODEL.md`](THREAT_MODEL.md) |
| Dependency audit | CI `deny` job + `.github/workflows/security.yml` (`cargo deny`, `cargo audit`) |
| Fail-closed offline / digests | Sprint 3–5 gates; VM asset SHA-256 pins |
| Capabilities / RO rootfs defaults | Security rules + runtime defaults |
| Unsafe review | All `unsafe` blocks require `// SAFETY:`; audited at GA cut |
| Privileged Linux suite | `#[ignore]` tests remain operator-run on privileged hosts; not a blocker for documented Linux support |

## G9.2 Performance sign-off

| Check | Evidence |
|---|---|
| Parse / resolve budgets | `containust-compose` `perf_regression` |
| Image import budgets | `containust-image` `perf_regression` |
| Documented budgets | [`PERFORMANCE.md`](PERFORMANCE.md) |
| Startup / stop | Platform smoke (`qemu-smoke-macos`); Linux native operator-validated |

## G9.3 Support policy

Published in [`SUPPORT_POLICY.md`](SUPPORT_POLICY.md).

## G9.4 GA release steps

1. Bump workspace to `1.0.0`; update CHANGELOG / README / CLI+SDK banners.
2. Green: `fmt`, `clippy -D warnings`, `test --workspace`, `deny check`.
3. Tag `v1.0.0` and push (triggers release artifacts + checksums).
4. Verify release assets; note signing still deferred (`PACKAGING.md`).
5. Archive this checklist as completed in `roadmap.md` / `work.md`.
