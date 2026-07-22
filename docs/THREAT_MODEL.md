# Containust Threat Model

**Version:** 0.5.0 (Sprint 4)  
**Status:** Living document — update when isolation, image, or persistence boundaries change.

## 1. Assets

| Asset | Why it matters |
| --- | --- |
| Host filesystem | Escape via malicious images, bind mounts, or symlink races writes host data. |
| Host kernel / privileges | Capability retention or missing `NO_NEW_PRIVS` enables privilege escalation. |
| Project state (`.containust/`) | Corrupted or secret-bearing state leaks credentials and breaks lifecycle. |
| Image layer store | Poisoned layers become rootfs for every subsequent run. |
| Operator secrets | Env vars such as `DB_PASSWORD` must not appear in state, logs, or plans. |

## 2. Trust boundaries

```
Operator / CI
    │  .ctst, CLI flags, host env, CONTAINUST_SECRET_*
    ▼
ctst (unprivileged or root)
    │  parse → plan → import → create/start
    ▼
Project store (.containust/{images,layers,state,logs,rootfs})
    │
    ▼
Linux spawn path (namespaces → mounts → pivot_root → caps → exec)
    │
    ▼
Container process (least privilege, optional cgroup limits)
```

Remote image fetch (`https://`, `preset://`) is an explicit trust boundary: offline mode must reject it before any socket opens; online fetch requires a pinned digest.

## 3. Adversaries

1. **Malicious image author** — crafts tar entries (`../`, absolute paths, symlink escapes, device nodes) to write outside the extraction root.
2. **Hostile composition author** — supplies volume specs that traverse into `/etc` or other host paths.
3. **Local multi-tenant peer** — shares a host and tries to read another project's state/logs or exceed resource quotas.
4. **Supply-chain attacker** — tampers with a remote archive in transit (mitigated by pinned SHA-256).
5. **Compromised container process** — tries to escalate via retained capabilities, setuid binaries, or host mounts.

## 4. Controls (mapped to Sprint 4)

| ID | Control | Status |
| --- | --- | --- |
| S4.1 | Safe archive extraction rejects traversal, absolute paths, unsafe types, escaping symlinks (including chained-symlink resolve-under-root) | Implemented (`containust-image::extract` + `path_confine`) |
| S4.2 | Drop all capabilities by default; `PR_SET_NO_NEW_PRIVS`; fail closed on drop errors; no `CAP_SYS_ADMIN` | Implemented |
| S4.3 | Explicit `NamespaceConfig`; mount/network/IPC/UTS on by default; unsupported PID/user requests fail closed | Implemented (PID/user deferred) |
| S4.4 | Volume specs validated in the parent (absolute, no `..`, canonicalize existing sources) | Implemented |
| S4.5 | Explicit memory/CPU limits validated and applied fail-closed via cgroups v2 | Implemented |
| S4.6 | Secret-looking env values redacted in `state.json`; restored from host / `CONTAINUST_SECRET_*` at start | Implemented |
| S4.7 | This threat model; `cargo deny` + `cargo audit` in CI (`.github/workflows/security.yml`) | Implemented |

## 5. Known limitations (accepted risk)

- **PID and user namespaces** are not yet applied on the spawn path (double-fork / uid maps pending). Requesting them fails closed rather than silently ignoring.
- **OverlayFS** is available as a library primitive but the Linux backend still materializes a copied/extracted rootfs.
- **Privileged Linux fixtures** that prove effective capability sets and cgroup enforcement require a supported host (or privileged CI job) and remain `#[ignore]` in the default suite.
- **VM backend** (macOS/Windows) is a separate trust boundary covered in Sprint 5.

## 6. Review checklist

Before calling a security sprint done:

- [x] Negative tests exist for unsafe tar entries and volume escapes.
- [x] Capability drop errors abort container start.
- [x] Requested cgroup limits that cannot be applied abort start and kill the process.
- [x] `state.json` fixtures contain no raw secret values (redacted at create).
- [x] `cargo deny check` and `cargo audit` are green.
- [x] This document matches runtime behavior.
