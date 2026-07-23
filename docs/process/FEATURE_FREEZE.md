# Feature Freeze Policy (Sprint 8 / beta)

After the `0.9.0-beta` tag, **no new runtime features** land until `1.0.0`.

## Allowed after freeze

- Correctness and security fixes
- Performance fixes that do not change external APIs
- Documentation, runbooks, and packaging metadata
- Test coverage and CI hardening
- Dependency updates required for advisories (`cargo deny` / audit)

## Disallowed after freeze

- New CLI subcommands or flags that change operator workflows
- New `.ctst` keywords or semantics
- New SDK public types/methods (except bugfix–driven narrow amendments)
- New backends or network models
- Behavioral changes to state schema without a migration rehearsal

## Process

1. Label candidate PRs `freeze-exception` only for P0/P1 security/correctness.
2. Keep [`CHANGELOG.md`](../CHANGELOG.md) accurate for every beta → RC → GA bump.
3. Compatibility expectations remain those in [`VERSIONING.md`](VERSIONING.md).
