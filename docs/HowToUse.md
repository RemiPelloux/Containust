# How to Use Containust

A short operator guide for day-to-day work with `ctst`. For deep dives, see
[CLI_REFERENCE.md](CLI_REFERENCE.md), [CTST_LANG.md](CTST_LANG.md), and
[TUTORIALS.md](TUTORIALS.md).

**Version:** 1.1.0

---

## 1. Install

Prefer a verified release binary ([PACKAGING.md](PACKAGING.md)):

```bash
VERSION=1.1.0
TARGET=x86_64-unknown-linux-gnu   # or aarch64-unknown-linux-gnu, *-apple-darwin, …
curl -LO "https://github.com/RemiPelloux/Containust/releases/download/v${VERSION}/ctst-${TARGET}.tar.gz"
curl -LO "https://github.com/RemiPelloux/Containust/releases/download/v${VERSION}/ctst-${TARGET}.tar.gz.sha256"
sha256sum -c "ctst-${TARGET}.tar.gz.sha256"
tar xzf "ctst-${TARGET}.tar.gz"
sudo install -m 755 ctst /usr/local/bin/ctst
ctst --version
```

Other options:

| Channel | Command / location |
| --- | --- |
| Debian / Ubuntu | `sudo dpkg -i ctst_1.1.0_amd64.deb` |
| Fedora / RHEL | `sudo rpm -i ctst-1.1.0-1.amd64.rpm` |
| Homebrew (in-tree) | `brew install --formula ./Formula/ctst.rb` |
| From source | `cargo install --path crates/containust-cli` |

macOS and Windows need **QEMU 7+** (`brew install qemu` / install QEMU on Windows).
Optional Cosign verification of `SHA256SUMS` is in [RUNBOOKS.md](RUNBOOKS.md).

---

## 2. Mental model (2 minutes)

- **No daemon.** Each `ctst` command talks to the kernel (Linux) or a QEMU VM
  (macOS/Windows) and writes state under `.containust/` next to your composition.
- **Compositions are `.ctst` files.** They declare `COMPONENT`s and `CONNECT`
  dependencies — similar to Compose, but with a smaller grammar.
- **Images are content-addressed.** Remote pulls land as
  `image://name@sha256:…`. Offline runs only use what is already in the catalog.
- **Reconciliation is daemonless.** `ctst ps` / `ctst run` apply restart and
  healthcheck policies when you invoke them.

---

## 3. First container in five commands

```bash
# 1) Pull a small image into the local catalog (digest-pinned)
ctst pull alpine:3.21
# prints something like: image://library/alpine@sha256:…

# 2) Write a composition (or use examples/alpine_preset.ctst)
cat > hello.ctst <<'EOF'
COMPONENT app {
    image   = "preset://alpine"
    command = ["/bin/busybox", "echo", "hello from Containust"]
}
EOF

# 3) Validate + import images
ctst plan hello.ctst
ctst build hello.ctst

# 4) Run (foreground) or detach
ctst run hello.ctst
# or: ctst run hello.ctst --detach

# 5) Inspect
ctst ps --all
ctst logs app
ctst stop app
ctst rm app
```

`preset://alpine` downloads a pinned Alpine minirootfs (~4 MiB) on first use.
`ctst pull` is for arbitrary Docker Hub / GHCR images.

---

## 4. Everyday workflow

| Goal | Command |
| --- | --- |
| See deploy order | `ctst plan stack.ctst` |
| Import / resolve images | `ctst build stack.ctst` |
| Dry-run import | `ctst build stack.ctst --dry-run` |
| Pull OCI image | `ctst pull nginx:alpine` |
| List catalog / presets | `ctst images` / `ctst images --presets` |
| Start stack | `ctst run stack.ctst --detach` |
| List containers | `ctst ps --all` |
| Follow logs | `ctst logs app --follow` |
| Exec into a container | `ctst exec app -- /bin/sh` |
| Stop / remove | `ctst stop app` then `ctst rm app` |
| Convert Compose | `ctst convert docker-compose.yml > stack.ctst` |
| Pre-boot VM (macOS/Windows) | `ctst vm start` |

Project data lives next to the composition:

```text
.containust/
  state/state.json    # schema v3 lifecycle state
  logs/<id>.log       # detached stdout/stderr
  images/             # catalog
  layers/<sha256>/    # content-addressed layers
```

Use a separate root with `--state-file /path/state.json` (or
`CONTAINUST_STATE_FILE`).

---

## 5. Images

### Sources

| Source | Example | Notes |
| --- | --- | --- |
| Preset | `preset://alpine` | Curated, pinned; also `busybox` |
| OCI pull | `ctst pull redis:7` then use printed `image://…@sha256:…` | Hub + GHCR |
| OCI URI | `oci://ghcr.io/org/app:1.0` | Same pull path via `build`/`import` |
| Local dir | `file:///opt/images/api` | Absolute path |
| Tar archive | `tar:///opt/images/api.tar` | e.g. `docker save` output |
| Catalog pin | `image://library/alpine@sha256:…` | Offline-safe |

### Auth for private registries

```bash
export CONTAINUST_REGISTRY_TOKEN=…          # bearer
# or
export CONTAINUST_REGISTRY_USER=…
export CONTAINUST_REGISTRY_PASSWORD=…
# or rely on ~/.docker/config.json
ctst pull ghcr.io/myorg/private:1.2.3
```

Credentials are never written to `state.json` or logs.

### Offline / air-gapped

```bash
# On a networked machine
ctst pull alpine:3.21
ctst build stack.ctst
# Copy .containust/images and .containust/layers to the air-gapped host

# On the air-gapped host
ctst --offline run stack.ctst --detach
```

`--offline` (or `CONTAINUST_OFFLINE=1`) rejects any remote fetch before connecting.

---

## 6. Ports, restart, healthchecks

### Publish a port

Identity publish and host:container remapping are both supported:

```text
COMPONENT web {
    image = "image://library/nginx@sha256:…"
    port  = 8080
    ports = ["8080"]
    network = "bridge"   # optional; default for remaps
}

EXPOSE 8080
EXPOSE 80:8080           # host 80 → container 8080
```

- **Linux (identity, no `network`):** host network namespace; bind directly
  (root or `CAP_NET_BIND_SERVICE` for ports &lt; 1024).
- **Linux (remap / named network):** private or shared netns + userspace
  forwarder; peers on the same network resolve via `/etc/hosts` → `127.0.0.1`.
- **macOS / Windows:** QEMU `hostfwd` (remap supported); adding ports to a live
  VM needs `ctst vm stop` and redeploy.

### Restart policy

```text
COMPONENT worker {
    image   = "preset://alpine"
    command = ["/bin/busybox", "sleep", "3600"]
    restart = "on-failure"    # never | on-failure | always
}
```

Policies are enforced when you run `ctst ps` or `ctst run` (no background
supervisor daemon).

### Healthcheck

```text
COMPONENT api {
    image = "image://library/myapi@sha256:…"
    port  = 8080
    healthcheck = {
        command     = ["wget", "-qO-", "http://127.0.0.1:8080/health"]
        interval    = "10s"
        timeout     = "3s"
        retries     = 3
        start_period = "5s"
    }
    restart = "on-failure"
}
```

See `examples/healthcheck_example.ctst` for a full sample.

---

## 7. Multi-service stack sketch

```text
COMPONENT db {
    image   = "image://library/postgres@sha256:…"
    port    = 5432
    env     = { POSTGRES_PASSWORD = "secret" }
    volumes = ["/data/pg:/var/lib/postgresql/data"]
    restart = "always"
}

COMPONENT api {
    image   = "file:///opt/images/api"
    port    = 8080
    ports   = ["8080"]
    memory  = "256MiB"
    restart = "on-failure"
}

CONNECT api -> db
EXPOSE 8080
```

`CONNECT` injects `DB_HOST`, `DB_PORT`, and related env vars into `api`.
Volumes use **host:container** bind mounts (no Docker named volumes).

---

## 8. Platform notes

| Host | Backend | Tips |
| --- | --- | --- |
| Linux 5.10+ | Native namespaces + cgroups v2 | User + PID namespaces on spawn (container init is PID 1); root or delegated userns recommended |
| macOS | QEMU + agent | `brew install qemu`; first run downloads Alpine VM assets to `~/.containust/cache/` |
| Windows | QEMU + agent | Install QEMU; same asset cache under the user profile |

```bash
# Optional: warm the VM before the first run
ctst vm start
ctst vm stop
```

---

## 9. Troubleshooting

| Symptom | What to try |
| --- | --- |
| `offline mode rejects remote source` | Drop `--offline`, or pre-pull / copy catalog layers |
| Port forward bind fails | Free the host port, or stop conflicting forwarders / QEMU hostfwd |
| Container `failed` right after `--detach` | `ctst logs <name>` — command may have exited; check image has the binary |
| macOS/Windows hang on first run | Ensure QEMU is installed; check serial/boot logs; increase `CONTAINUST_VM_BOOT_TIMEOUT_SECS` |
| Permission denied on Linux | Run with sufficient privileges for namespaces/cgroups; confirm cgroup v2 |
| Private registry 401 | Set `CONTAINUST_REGISTRY_*` or refresh `~/.docker/config.json` |
| Stale `Running` entries | `ctst ps` reconciles dead PIDs; then `ctst rm --force` if needed |

Doctor / diagnostics:

```bash
ctst doctor          # if available in your build
ctst --help
```

---

## 10. Where to go next

| Doc | When |
| --- | --- |
| [TUTORIALS.md](TUTORIALS.md) | Step-by-step lessons (hello → full stack → offline → SDK) |
| [CLI_REFERENCE.md](CLI_REFERENCE.md) | Every flag and subcommand |
| [CTST_LANG.md](CTST_LANG.md) | Full `.ctst` grammar |
| [MIGRATION_FROM_DOCKER.md](MIGRATION_FROM_DOCKER.md) | Coming from Compose |
| [SUPPORT_POLICY.md](SUPPORT_POLICY.md) | What is supported vs deferred |
| [PACKAGING.md](PACKAGING.md) | Install channels and signing |
| [examples/](../examples/) | Ready-made compositions |

Quick examples to try:

```bash
ctst plan examples/alpine_preset.ctst
ctst plan examples/healthcheck_example.ctst
ctst convert path/to/docker-compose.yml
```
