# Beta Release Candidate Checklist (`0.9.0-beta`)

Sprint 8 / B8.4. Feature freeze is in effect ([FEATURE_FREEZE.md](FEATURE_FREEZE.md)).

## Cut the candidate

1. Workspace version is `0.9.0-beta.1` (or later `0.9.0-beta.N`).
2. `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo deny check` are green.
3. Compatibility + upgrade rehearsal tests pass:
   - `cargo test -p containust-runtime --test compat_matrix`
   - `cargo test -p containust-runtime --test upgrade_rehearsal`
4. Tag and push: `git tag v0.9.0-beta.1 && git push origin v0.9.0-beta.1`
5. Confirm GitHub Release artifacts + `.sha256` files for all five targets.

## Clean-machine install matrix

Perform **two** independent installs per platform (different machines or VMs):

| Platform | Artifact | Verify |
|---|---|---|
| Linux x86_64 | `ctst-x86_64-unknown-linux-gnu.tar.gz` | `sha256sum -c` → extract → `./ctst --version` → `./ctst doctor` |
| Linux aarch64 | `ctst-aarch64-unknown-linux-gnu.tar.gz` | same |
| macOS arm64 | `ctst-aarch64-apple-darwin.tar.gz` | same (+ QEMU for `vm start` smoke) |
| macOS x86_64 | `ctst-x86_64-apple-darwin.tar.gz` | same |
| Windows x86_64 | `ctst-x86_64-pc-windows-msvc.zip` | Get-FileHash → expand → `ctst.exe --version` |

Record results in the release notes (pass/fail + machine id). Failures block GA.

## Exit

Beta is accepted when both install runs per platform succeed and no P0/P1 issues remain open.
