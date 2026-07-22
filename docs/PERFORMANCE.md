# Performance Budgets

Regression gates run in CI via `cargo test`. Budgets are generous enough for
slow runners but tight enough to catch accidental multi-pass work.

| Surface | Test | Budget | Location |
|---|---|---|---|
| Image import (32 MiB dir) | `import_directory_32mib_completes_within_budget` | 5s | `crates/containust-image/tests/perf_regression.rs` |
| Image import (32 MiB tar) | `import_tar_32mib_completes_within_budget` | 5s | same |
| `.ctst` parse (wide graph) | `parse_wide_composition_within_budget` | 200ms | `crates/containust-compose/tests/perf_regression.rs` |
| Graph resolve (wide graph) | `resolve_wide_composition_within_budget` | 200ms | same |

Startup/teardown of real containers remain platform-dependent (namespaces /
QEMU) and are covered by smoke jobs rather than fixed wall-clock unit budgets.

When changing import, parse, or resolve paths, run:

```bash
cargo test -p containust-image --test perf_regression
cargo test -p containust-compose --test perf_regression
```
