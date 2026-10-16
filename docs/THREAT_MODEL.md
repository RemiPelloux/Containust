# Containust Threat Model

**Version:** 1.5.0 (reviewed after Sprint 13 hardening)
**Status:** Living document — update when isolation, image, networking, or persistence boundaries change.

## 1. Assets

| Asset | Why it matters |
| --- | --- |
| Host filesystem | Escape via malicious images, bind mounts, or symlink races writes host data. |
| Host kernel / privileges | Capability retention or missing `NO_NEW_PRIVS` enables privilege escalation. |
| Project state (`.containust/`) | `state.json`, `applied.json`, port target links and netns mounts drive every lifecycle decision; corruption breaks or hijacks running services. |
| Image layer store | Poisoned layers become rootfs for every subsequent run. |
| Operator secrets | Env vars such as `DB_PASSWORD` must not appear in state, logs, or plans. |
| Published services | Host ports relayed into containers; availability during updates is a promise (`start-first`). |

## 2. Trust boundaries

```
Operator / CI
    │  .ctst (+ local IMPORTs), CLI flags, host env, CONTAINUST_SECRET_*
    ▼
ctst (root or delegated userns) ──── registry / preset downloads (opt-in, digest-pinned)
    │  parse → resolve templates → plan → import → create/start → publish
    ▼
Project store (.containust/{images,layers,state,logs,rootfs,networks,ports})
    │
    ├─▶ Linux spawn path (user+PID+mount+net+IPC+UTS ns → pivot_root → caps drop → exec)
    │        ▼
    │   Container process (least privilege, optional cgroup limits)
    │
    └─▶ Port forwarders (detached helpers, host 127.0.0.1:<port> → container netns)
```

- Remote sources (`oci://`, `https://`, `preset://`) are an explicit boundary:
  `--offline` rejects them before any socket opens; raw downloads need a pinned
  digest; `ctst pull --require-provenance` adds cosign verification.
- `IMPORT` is local-only: remote imports fail closed. Imported files are
  trusted like the importing file (they are part of the composition).
- On macOS/Windows the same requests cross a versioned JSON-RPC boundary into
  an agent inside a pinned Alpine QEMU VM.

## 3. Adversaries

1. **Malicious image author** — crafts tar entries (`../`, absolute paths, symlink escapes, device nodes) to write outside the extraction root.
2. **Hostile composition author** — supplies volume specs that traverse into `/etc`, or import cycles / deep chains to exhaust the parser.
3. **Local unprivileged user** — on the same host; can connect to published ports on `127.0.0.1` and try to exhaust forwarder resources.
4. **Supply-chain attacker** — tampers with a remote archive or registry blob in transit (mitigated by pinned SHA-256 and optional provenance).
5. **Compromised container process** — tries to escalate via retained capabilities, setuid binaries, or host mounts.
6. **Concurrent or interrupted operator** — two `ctst run`s at once, or a run killed mid-update, leaving state inconsistent.

## 4. Controls

| ID | Control | Status |
| --- | --- | --- |
| C1 | Safe archive extraction rejects traversal, absolute paths, unsafe types, escaping symlinks (including chained-symlink resolve-under-root) | Implemented (`containust-image::extract` + `path_confine`) |
| C2 | Drop all capabilities by default; `PR_SET_NO_NEW_PRIVS`; fail closed on drop errors | Implemented |
| C3 | User + PID namespaces on by default (container init is PID 1, uid-mapped); mount/net/IPC/UTS isolated; unsupported namespace requests fail closed | Implemented (`1.2.0`) |
| C4 | Volume specs validated in the parent (absolute, no `..`, canonicalize existing sources) | Implemented |
| C5 | Memory/CPU limits validated and applied fail-closed via cgroups v2 | Implemented |
| C6 | Secret-looking env values redacted in `state.json`; restored from host / `CONTAINUST_SECRET_*` at start | Implemented |
| C7 | Digest-pinned images; `--offline` fail-closed; registry credentials never persisted; optional cosign provenance | Implemented (`1.1.0`) |
| C8 | Forwarded ports (remaps, private and shared networks) bind `127.0.0.1` only and relay into the target netns with `setns`, no `CAP_NET_ADMIN`/nftables. Host-network components bind wherever the application binds. | Implemented (`1.2.0`) |
| C9 | Forwarder isolation: detached session, stdio on `/dev/null`, every inherited descriptor closed except the listener (no state-lock or log fds pinned) | Implemented (`1.5.0`) |
| C10 | Forwarder resource bounds: at most 512 concurrent relays per port (further clients wait in the backlog); handlers reaped; ports bound before any target link changes | Implemented (`1.5.0`) |
| C11 | Port target links (`.containust/ports/<host>`) are swapped atomically (temp symlink + rename) and point at canonical netns paths | Implemented (`1.4.0`) |
| C12 | Project-wide `apply.lock` serializes `ctst run`; state writes are atomic under a state lock | Implemented (`1.5.0`) |
| C13 | Start-first updates are crash-safe: the staged container is recorded after the switch and the next run completes (or discards) it; a failed switch returns the ports to the old container | Implemented (`1.5.0`) |
| C14 | Health probes run with a timeout (process group killed) and the start-first gate has an overall deadline; durations capped at 24h | Implemented (`1.5.0`) |
| C15 | Composition limits: import nesting and `FROM` chains ≤ 16, cycles rejected, duplicate host ports rejected at plan time | Implemented (`1.5.0`) |
| C16 | `cargo deny` + `cargo audit` in CI (`.github/workflows/security.yml`); CodeQL | Implemented |

## 5. Known limitations (accepted risk)

- **Forwarders fork from the calling process.** The `ctst` CLI is
  single-threaded when it forks; an SDK host that applies compositions with
  published ports from a heavily multithreaded process relies on glibc's
  fork-safety (malloc locks are reset). A dedicated helper process is planned.
- **Forwarder liveness uses PIDs.** A forwarder that died long ago and whose
  PID was recycled can be mistaken for alive during a handoff; the switch then
  repoints a link nobody serves. `ctst run` reports the port as published.
- **Anyone on the host can reach published ports** on `127.0.0.1` — the same
  as any loopback service — and host-network components are reachable on
  whatever address the application binds. Use host firewalling or do not publish.
- **Start-first overlap shares volumes**: old and new containers mount the same
  host paths during the drain. Keep `recreate` for exclusive-writer stores.
- **Shared networks are one namespace**: members can reach every port of every
  other member. Use separate network names to separate trust zones.
- **OverlayFS** is available as a library primitive but the Linux backend still materializes a copied/extracted rootfs.
- **Privileged fixtures** (spawn, forwarders, zero-downtime) need root and run in the privileged CI job; they are `#[ignore]` in the default suite.

## 6. Review checklist

Before calling a security-relevant sprint done:

- [x] Negative tests exist for unsafe tar entries and volume escapes.
- [x] Capability drop errors abort container start.
- [x] Requested cgroup limits that cannot be applied abort start and kill the process.
- [x] `state.json` fixtures contain no raw secret values (redacted at create).
- [x] Forwarders hold only stdio + listener (`forwarder_netns` fixture).
- [x] An interrupted or unhealthy start-first update never leaves the service without a serving container (engine tests + `zero_downtime` gate).
- [x] `cargo deny check` and `cargo audit` are green.
- [x] This document matches runtime behavior.
