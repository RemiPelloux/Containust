# Performance Budgets

Regression gates run in CI via `cargo test`. Budgets are generous enough for
slow runners but tight enough to catch accidental multi-pass work.

| Surface | Test | Budget | Location |
|---|---|---|---|
| Image import (32 MiB dir) | `import_directory_32mib_completes_within_budget` | 5s | `crates/containust-image/tests/perf_regression.rs` |
| Image import (32 MiB tar) | `import_tar_32mib_completes_within_budget` | 5s | same |
| Extract 10k small files (tar.gz) | `extract_many_small_files_completes_within_budget` | 20s | same |
| Stop: SIGTERM-responsive process | `process::tests::terminate_returns_promptly_when_process_exits_on_sigterm` | 1s (grace is 2s) | `crates/containust-runtime/src/process.rs` |
| `.ctst` parse (wide graph) | `parse_wide_composition_within_budget` | 200ms | `crates/containust-compose/tests/perf_regression.rs` |
| Graph resolve (wide graph) | `resolve_wide_composition_within_budget` | 200ms | same |

Stop polls for exit every 20ms during the 2s SIGTERM grace period instead of
sleeping the full grace, and the Linux backend signals/waits outside the
exclusive state lock. Rootfs preparation in `create` also runs outside the
lock (built under `rootfs-staging/<id>`, renamed into `rootfs/<id>` on commit).
OCI pulls skip downloading layers already present in the content-addressed
store with the descriptor's size.

Startup/teardown of real containers remain platform-dependent (namespaces /
QEMU) and are covered by smoke jobs rather than fixed wall-clock unit budgets.

When changing import, parse, or resolve paths, run:

```bash
cargo test -p containust-image --test perf_regression
cargo test -p containust-compose --test perf_regression
cargo test -p containust-runtime --lib process::tests
```
