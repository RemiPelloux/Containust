# ctst — Containust CLI Reference

**Binary**: `ctst`
**Version**: 0.1.0
**Platform**: Linux (kernel 5.10+)

```
ctst [OPTIONS] <COMMAND> [ARGS...]
```

Containust is a daemon-less, sovereign container runtime written in Rust. The `ctst` binary is the single entry point for building, deploying, inspecting, and managing containers defined in `.ctst` composition files. It communicates with the kernel directly via syscalls — no background daemon required.

---

## Installation

### From Source

```bash
git clone https://github.com/RemiPelloux/Containust.git
cd Containust
cargo install --path crates/containust-cli
```

### Verify

```bash
ctst --version
# ctst 0.1.0
```

---

## Global Options

These flags apply to every subcommand.

| Flag | Description | Default | Env Override |
|---|---|---|---|
| `--offline` | Block all outbound network access during build and run | `false` | `CONTAINUST_OFFLINE=1` |
| `--state-file <PATH>` | Path to the state index file | `/var/lib/containust/state.json` | `CONTAINUST_STATE_FILE` |
| `--help` | Print help information and exit | — | — |
| `--version` | Print version information and exit | — | — |

Tracing verbosity is controlled by the `CONTAINUST_LOG` environment variable, which accepts
[`tracing` filter directives](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html)
(e.g., `info`, `containust_runtime=debug`).

---

## ctst build

Parse a `.ctst` composition file, resolve imports, download or locate images, and assemble filesystem layers.

### Synopsis

```
ctst build [OPTIONS] [FILE]
```

### Arguments

| Argument | Description | Default |
|---|---|---|
| `FILE` | Path to the `.ctst` composition file | `containust.ctst` |

### Options

Inherits all [global options](#global-options).

### Description

`ctst build` performs the following steps:

1. **Parse** the `.ctst` file using the nom-based parser.
2. **Resolve imports** — recursively load any `IMPORT` directives.
3. **Fetch images** — download or locate images from `file://`, `tar://`, or remote sources. Validates every layer with SHA-256.
4. **Assemble layers** — build OverlayFS layer stacks and store them in the image store.
5. **Analyze binaries** — run distroless dependency analysis when enabled.

Built layers are cached. Subsequent builds skip layers whose content hash has not changed.

### Output Format

```
$ ctst build webapp.ctst
⠋ Parsing webapp.ctst...
✓ Parsed 3 components, 2 connections

⠋ Resolving images...
  [1/3] api        file:///opt/images/myapp    sha256:a1b2c3d4...  42.8 MiB
  [2/3] db         tar:///backups/pg15.tar      sha256:e5f6a7b8...  89.1 MiB
  [3/3] cache      file:///opt/images/redis     sha256:c9d0e1f2...  12.3 MiB
✓ All images resolved (144.2 MiB total)

⠋ Building layers...
  Layer sha256:a1b2c3d4  → 3 sub-layers  42.8 MiB  (cached)
  Layer sha256:e5f6a7b8  → 5 sub-layers  89.1 MiB  (built in 1.4s)
  Layer sha256:c9d0e1f2  → 2 sub-layers  12.3 MiB  (cached)
✓ Build complete (1.4s)
```

### Layer Caching

Layers are stored under `/var/lib/containust/images/` keyed by their SHA-256 content hash. A layer is rebuilt only when its source content changes. Use `--offline` to restrict builds to locally cached layers only.

### Exit Codes

| Code | Meaning |
|---|---|
| `0` | Build succeeded |
| `1` | Build failed (parse error, missing image, hash mismatch) |
| `2` | Invalid arguments |
| `4` | Source file or image not found |

### Examples

```bash
# Build from the default containust.ctst in the current directory
ctst build

# Build from a specific file
ctst build infrastructure/production.ctst

# Build in offline mode (no network access — cached layers only)
ctst build --offline

# Build with a custom state file location
ctst build --state-file /tmp/dev-state.json webapp.ctst
```

---

## ctst plan

Display the planned infrastructure changes without applying them (dry run).

### Synopsis

```
ctst plan [OPTIONS] [FILE]
```

### Arguments

| Argument | Description | Default |
|---|---|---|
| `FILE` | Path to the `.ctst` composition file | `containust.ctst` |

### Options

Inherits all [global options](#global-options).

### Description

`ctst plan` compares the desired state described in the `.ctst` file against the current state recorded in the state file. It outputs a diff-style summary showing what would change if `ctst run` were executed.

No containers are created, started, or stopped.

### Output Format

The output uses diff-style markers:

| Marker | Meaning |
|---|---|
| `+` | New component to be created |
| `~` | Existing component to be modified |
| `-` | Running component to be removed |

```
$ ctst plan production.ctst

Containust Plan — production.ctst
══════════════════════════════════

  + api          image=file:///opt/images/myapp     port=8080  mem=256MB
  + db           image=tar:///backups/pg15.tar      port=5432  mem=512MB
  ~ cache        image=file:///opt/images/redis     mem: 64MB → 128MB
  - legacy-svc   (will be stopped and removed)

Plan: 2 to add, 1 to change, 1 to destroy.
```

### Exit Codes

| Code | Meaning |
|---|---|
| `0` | Plan computed — changes detected or no changes |
| `1` | Failed to compute plan (parse error, corrupt state) |
| `2` | Invalid arguments |
| `4` | Composition file not found |

### Examples

```bash
# Plan using the default file
ctst plan

# Plan a specific composition
ctst plan staging.ctst

# Plan in offline mode
ctst plan --offline production.ctst

# Plan with verbose tracing
CONTAINUST_LOG=debug ctst plan
```

---

## ctst run

Deploy the component graph defined in a `.ctst` composition file.

### Synopsis

```
ctst run [OPTIONS] [FILE]
```

### Arguments and Options

| Argument / Flag | Description | Default |
|---|---|---|
| `FILE` | Path to the `.ctst` composition file | `containust.ctst` |
| `-d, --detach` | Run containers in the background and return immediately | `false` |

Inherits all [global options](#global-options).

### Description

`ctst run` performs a full deployment:

1. **Parse and resolve** the `.ctst` file (equivalent to `ctst build` if images are not yet built).
2. **Compute the dependency graph** from `CONNECT` directives using petgraph.
3. **Topological sort** to determine startup order. Components with no dependencies start in parallel.
4. **Create containers** — set up namespaces (PID, mount, network, IPC, UTS), cgroups v2 resource limits, and OverlayFS mounts.
5. **Start processes** in the correct order, injecting connection environment variables from `CONNECT` wiring.
6. **Update the state file** with container metadata.

### Startup Behavior

Containers are started in topological order derived from `CONNECT` directives. Independent components (no inbound or outbound connections) start in parallel. A component only starts after all of its dependencies report a healthy state.

### Detached Mode

When `-d` / `--detach` is passed, `ctst run` daemonizes the container processes and returns immediately. The state file is updated and containers continue running in the background. Use `ctst ps` to monitor and `ctst stop` to shut down.

Without `--detach`, `ctst run` remains in the foreground, streaming logs to stdout. Press `Ctrl+C` to initiate graceful shutdown.

### Output Format

```
$ ctst run -d production.ctst

Containust Deploy — production.ctst
════════════════════════════════════

  ✓ db           started  pid=48201  port=5432  mem_limit=512MB
  ✓ cache        started  pid=48215  port=6379  mem_limit=128MB
  ✓ api          started  pid=48230  port=8080  mem_limit=256MB

All 3 containers running (detached).
State saved to /var/lib/containust/state.json
```

### Exit Codes

| Code | Meaning |
|---|---|
| `0` | All containers started successfully |
| `1` | One or more containers failed to start |
| `2` | Invalid arguments |
| `3` | Permission denied (insufficient privileges for namespace/cgroup operations) |
| `4` | Composition file or image not found |

### Examples

```bash
# Run the default composition in the foreground
ctst run

# Run in detached mode
ctst run -d

# Run a specific file, detached, in offline mode
ctst run -d --offline production.ctst

# Run with debug logging
CONTAINUST_LOG=debug ctst run

# Run with a custom state file
ctst run --state-file /tmp/dev-state.json dev.ctst
```

---

## ctst ps

List containers with their status and resource metrics.

### Synopsis

```
ctst ps [OPTIONS]
```

### Options

| Flag | Description | Default |
|---|---|---|
| `-a, --all` | Show all containers including stopped and failed | `false` |
| `--tui` | Launch the interactive TUI dashboard | `false` |

Inherits all [global options](#global-options).

### Description

`ctst ps` reads the state file and queries cgroups v2 for live resource metrics. By default it shows only running containers.

### Output Columns

| Column | Description | Example |
|---|---|---|
| `CONTAINER ID` | Truncated UUID (first 12 characters) | `a1b2c3d4e5f6` |
| `NAME` | Component name from the `.ctst` file | `api` |
| `STATE` | Current lifecycle state | `running` |
| `CPU%` | CPU usage percentage from cgroup stats | `2.3%` |
| `MEM USAGE` | Current memory consumption | `45.2 MiB` |
| `NET I/O` | Network bytes received / transmitted | `1.2 MiB / 340 KiB` |
| `UPTIME` | Time since container started | `2h 14m` |

### Container States

| State | Description |
|---|---|
| `created` | Container exists but process has not started |
| `running` | Process is active |
| `stopped` | Process exited normally (exit code 0) |
| `failed` | Process exited with a non-zero exit code |

### Metrics

CPU and memory values are read from the cgroup v2 unified hierarchy (`/sys/fs/cgroup`). Memory is displayed in human-readable units (B, KiB, MiB, GiB). CPU percentage is computed over a sampling window.

Network I/O is read from the container's network namespace counters.

### TUI Dashboard

When launched with `--tui`, an interactive terminal dashboard (powered by ratatui) takes over the terminal.

**Navigation keys:**

| Key | Action |
|---|---|
| `q` / `Esc` | Quit the dashboard |
| `Tab` | Switch between panels (containers, metrics, logs) |
| `↑` / `↓` | Scroll through container list |
| `←` / `→` | Cycle metric time ranges |
| `/` | Open search / filter |
| `Enter` | Expand selected container details |

### Exit Codes

| Code | Meaning |
|---|---|
| `0` | Listing succeeded |
| `1` | Failed to read state file or query metrics |
| `4` | State file not found |

### Examples

```bash
# List running containers
ctst ps

# List all containers, including stopped
ctst ps --all

# Launch the interactive TUI dashboard
ctst ps --tui

# Use a custom state file
ctst ps --state-file /tmp/dev-state.json --all
```

---

## ctst exec

Execute a command inside a running container by joining its Linux namespaces.

### Synopsis

```
ctst exec <CONTAINER> -- <COMMAND...>
```

### Arguments

| Argument | Description | Required |
|---|---|---|
| `CONTAINER` | Container ID (or prefix) or component name | Yes |
| `COMMAND...` | Command and arguments to execute inside the container | Yes |

Inherits all [global options](#global-options).

### Description

`ctst exec` joins the target container's Linux namespaces — **PID**, **mount**, **network**, **IPC**, and **UTS** — then executes the specified command within that isolated environment. The process sees the container's filesystem, network stack, and process tree.

### How Namespace Joining Works

1. **Lookup** — Resolve the container by ID or name from the state file.
2. **Open namespace file descriptors** — Read `/proc/<pid>/ns/{pid,mnt,net,ipc,uts}` for the container's init process.
3. **`setns()` syscall** — Join each namespace.
4. **`chroot` / `pivot_root`** — Enter the container's root filesystem.
5. **`execvp`** — Replace the current process with the requested command.

### Interactive vs Non-Interactive Mode

If the command is an interactive shell (e.g., `/bin/sh`, `/bin/bash`), `ctst exec` allocates a pseudo-TTY and attaches stdin/stdout/stderr for interactive use. Non-interactive commands run, print their output, and exit.

### Exit Codes

`ctst exec` forwards the exit code of the executed command. Additional codes:

| Code | Meaning |
|---|---|
| `0` | Command succeeded |
| `1` | General error (container not running, namespace join failed) |
| `4` | Container not found |
| `126` | Command found but cannot be executed (permission denied inside container) |
| `127` | Command not found inside the container |

### Examples

```bash
# Open an interactive shell
ctst exec api -- /bin/sh

# Run a one-off database query
ctst exec db -- psql -U postgres -c "SELECT version();"

# Check the filesystem inside a container
ctst exec cache -- ls -la /data

# Inspect environment variables
ctst exec api -- env

# Use a container ID prefix instead of name
ctst exec a1b2c3 -- cat /etc/hostname
```

---

## ctst stop

Stop one or more containers and clean up their associated resources.

### Synopsis

```
ctst stop [OPTIONS] [CONTAINERS...]
```

### Arguments and Options

| Argument / Flag | Description | Default |
|---|---|---|
| `CONTAINERS...` | Container IDs or names to stop | All running containers |
| `-f, --force` | Skip graceful shutdown — send `SIGKILL` immediately | `false` |

Inherits all [global options](#global-options).

### Description

`ctst stop` initiates a shutdown of the specified containers (or all running containers if none are specified).

### Graceful Shutdown Process

1. **SIGTERM** — Send `SIGTERM` to the container's init process.
2. **Grace period** — Wait up to **10 seconds** for the process to exit.
3. **SIGKILL** — If the process has not exited, send `SIGKILL`.

With `--force`, step 1 is skipped and `SIGKILL` is sent immediately.

### Resource Cleanup

After the process exits, `ctst stop` performs cleanup:

- **Cgroup removal** — Delete the container's cgroup directory from `/sys/fs/cgroup`.
- **Mount teardown** — Unmount OverlayFS layers and any bound volumes.
- **State file update** — Mark the container as `stopped` in the state index.
- **Network cleanup** — Remove virtual network interfaces.

### Exit Codes

| Code | Meaning |
|---|---|
| `0` | All specified containers stopped successfully |
| `1` | One or more containers failed to stop |
| `3` | Permission denied |
| `4` | Container not found |

### Examples

```bash
# Stop all running containers gracefully
ctst stop

# Stop specific containers by name
ctst stop api db

# Force kill a stuck container
ctst stop --force legacy-worker

# Stop a container by ID prefix
ctst stop a1b2c3
```

---

## ctst images

Manage the local image catalog.

### Synopsis

```
ctst images [OPTIONS]
```

### Options

| Flag | Description |
|---|---|
| `-l, --list` | List all locally stored images |
| `--remove <ID>` | Remove an image by its SHA-256 ID |

Inherits all [global options](#global-options).

### Description

`ctst images` provides operations on the local image store located at `/var/lib/containust/images/`. Images are composed of content-addressable OverlayFS layers identified by their SHA-256 hash.

### Output Format (--list)

```
$ ctst images --list

IMAGE ID                    SOURCE                          SIZE       CREATED              LAYERS
sha256:a1b2c3d4e5f6a7b8    file:///opt/images/myapp        42.8 MiB   2026-02-20 14:30    3
sha256:e5f6a7b8c9d0e1f2    tar:///backups/pg15.tar         89.1 MiB   2026-02-19 09:15    5
sha256:c9d0e1f2a3b4c5d6    file:///opt/images/redis        12.3 MiB   2026-02-18 22:00    2
```

### Image ID Format

Image IDs follow the `sha256:<hex>` convention. The full ID is 64 hex characters; short prefixes (minimum 12 characters) are accepted anywhere a full ID is required, provided they are unambiguous.

### Storage Location

Images and their layers are stored under `/var/lib/containust/images/`. Each image directory contains its layer tarballs and a manifest file linking layers to their content hashes.

### Exit Codes

| Code | Meaning |
|---|---|
| `0` | Operation succeeded |
| `1` | Operation failed (I/O error, image in use) |
| `4` | Image ID not found |

### Examples

```bash
# List all local images
ctst images --list

# Remove an image by full ID
ctst images --remove sha256:a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6

# Remove an image by short ID prefix
ctst images --remove sha256:a1b2c3d4e5f6

# List images using a custom state file
ctst images --list --state-file /tmp/dev-state.json
```

---

## State File

Containust manages container lifecycle through a local JSON state file instead of a daemon.

### Location

Default: `/var/lib/containust/state.json`
Override: `--state-file <PATH>` or `CONTAINUST_STATE_FILE` environment variable.

### Format

```json
{
  "version": 1,
  "containers": [
    {
      "id": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "name": "api",
      "state": "running",
      "pid": 48230,
      "created_at": "2026-02-24T10:30:00Z",
      "image": "sha256:a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6",
      "limits": {
        "memory_bytes": 268435456,
        "cpu_shares": 1024
      }
    },
    {
      "id": "f6a7b8c9-d0e1-4f2a-3b4c-5d6e7f8a9b0c",
      "name": "db",
      "state": "running",
      "pid": 48201,
      "created_at": "2026-02-24T10:29:58Z",
      "image": "sha256:e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0",
      "limits": {
        "memory_bytes": 536870912,
        "cpu_shares": 2048
      }
    }
  ]
}
```

### When the State File Is Updated

- **`ctst run`** — Entries added when containers are created.
- **`ctst stop`** — Entries updated to `stopped` state; PID cleared.
- **`ctst build`** — No state file changes (build is stateless).
- **Container exit** — On next `ctst ps` invocation, stale PIDs are detected and states corrected.

### Corruption Recovery

If the state file is corrupted or missing:

1. `ctst` creates a new empty state file on next invocation.
2. Orphaned containers (running processes with no state entry) can be discovered via `/sys/fs/cgroup` and re-registered.
3. Back up the state file before manual edits: `cp state.json state.json.bak`.

---

## Container Naming

### Container ID

Every container receives a **UUID v4** identifier at creation time (e.g., `a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d`). The full UUID is 36 characters including hyphens.

### Container Name

The container name is derived from the `COMPONENT` name in the `.ctst` file (e.g., `api`, `db`, `cache`). Names are unique within a single deployment.

### Referencing Containers

All commands that accept a container reference (`exec`, `stop`) resolve in the following order:

1. **Exact name match** — `api`
2. **UUID prefix match** — `a1b2c3d4` (minimum 8 characters, must be unambiguous)
3. **Full UUID match** — `a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d`

If a prefix is ambiguous, the command fails with an error listing the matching containers.

---

## Exit Codes

Summary of all exit codes used across `ctst` commands.

| Code | Meaning | Commands |
|---|---|---|
| `0` | Success | All |
| `1` | General error | All |
| `2` | Usage / argument error | All |
| `3` | Permission denied (namespace/cgroup creation) | `run`, `exec`, `stop` |
| `4` | Resource not found (file, image, container) | All |
| `126` | Command cannot execute (permission denied inside container) | `exec` |
| `127` | Command not found inside container | `exec` |

---

## Environment Variables

| Variable | Description | Default |
|---|---|---|
| `CONTAINUST_STATE_FILE` | Path to the state index file | `/var/lib/containust/state.json` |
| `CONTAINUST_LOG` | Tracing filter directive (e.g., `info`, `debug`, `containust_runtime=trace`) | `warn` |
| `CONTAINUST_OFFLINE` | Set to `1` to enable offline mode (equivalent to `--offline`) | unset |
| `CONTAINUST_DATA_DIR` | Base directory for all Containust data | `/var/lib/containust` |
| `CONTAINUST_IMAGE_STORE` | Directory for cached images and layers | `/var/lib/containust/images` |
| `CONTAINUST_ROOTFS_DIR` | Directory for container rootfs mounts | `/var/lib/containust/rootfs` |

CLI flags take precedence over environment variables.

---

## Troubleshooting

### "permission denied" when creating namespaces

**Cause**: Creating Linux namespaces requires either root privileges or unprivileged user namespace support.

**Fix**:
```bash
# Run with sudo
sudo ctst run

# Or enable unprivileged user namespaces (if supported by your kernel)
sudo sysctl -w kernel.unprivileged_userns_clone=1
```

### "cgroups v2 not available"

**Cause**: The kernel is not mounted with cgroups v2 unified hierarchy, or the system is using cgroups v1.

**Fix**:
```bash
# Check if cgroups v2 is mounted
mount | grep cgroup2

# If not, add to kernel boot params:
# systemd.unified_cgroup_hierarchy=1
# Then reboot.
```

### "state file locked"

**Cause**: Another `ctst` process is holding a lock on the state file, or a previous invocation crashed without releasing the lock.

**Fix**:
```bash
# Check for running ctst processes
ps aux | grep ctst

# If no processes are running, remove the stale lock
rm /var/lib/containust/state.json.lock
```

### "image not found"

**Cause**: The image source path in the `.ctst` file does not exist, or the image has not been built yet.

**Fix**:
```bash
# Verify the source path exists
ls -la /opt/images/myapp

# Build images first
ctst build

# Then run
ctst run
```

### Container won't stop

**Cause**: The container process is ignoring `SIGTERM` (e.g., it traps or ignores the signal).

**Fix**:
```bash
# Force kill the container
ctst stop --force stuck-container

# If that fails, find and kill the process manually
ctst ps --all  # note the PID from the state file
sudo kill -9 <PID>
```

### Port already in use

**Cause**: Another process (or a previously stopped container that was not cleaned up) is binding the requested port.

**Fix**:
```bash
# Find what is using the port
sudo ss -tlnp | grep :8080

# Stop the conflicting process, then retry
ctst run
```

### Out of memory

**Cause**: The container exceeded its cgroup memory limit and was killed by the OOM killer.

**Fix**:
```bash
# Increase the memory limit in your .ctst file
# memory = "512MB"  →  memory = "1GB"

# Check dmesg for OOM events
dmesg | grep -i "out of memory"
```

### "offline mode" blocking imports

**Cause**: `--offline` (or `CONTAINUST_OFFLINE=1`) is active and the `.ctst` file references remote imports or images that are not locally cached.

**Fix**:
```bash
# Build with network access first to populate the cache
ctst build

# Then run in offline mode
ctst run --offline

# Or remove --offline to allow network access
ctst run
```

---

## 11. `ctst convert`

Convert a `docker-compose.yml` file to Containust `.ctst` format.

### Synopsis

```bash
ctst convert [OPTIONS] [FILE]
```

### Description

Parses a Docker Compose YAML file and emits the equivalent `.ctst` composition language output. This provides a fast migration path from Docker Compose to Containust.

The converter handles:
- **Services** to `COMPONENT` blocks
- **`depends_on`** to `CONNECT` statements with auto-wiring
- **`ports`** to `port` / `ports` properties with `EXPOSE` comments
- **`volumes`** to `volume` / `volumes` properties
- **`environment`** to `env` maps (Docker `${}` vars mapped to `${secret.*}`)
- **`restart`** policies (`no` -> `"never"`, `unless-stopped` -> `"always"`)
- **`healthcheck`** to `healthcheck` blocks (strips `CMD`/`CMD-SHELL` prefixes)
- **`mem_limit`** / `deploy.resources.limits.memory` to `memory` with size conversion
- **`deploy.resources.limits.cpus`** to `cpu` shares (multiplied by 1024)
- **`command`**, `entrypoint`, `working_dir`, `user`, `hostname`, `read_only`, `networks`
- **Docker Hub images** converted to `tar://` placeholders with export instructions

### Arguments

| Argument | Description | Default |
|---|---|---|
| `FILE` | Path to the docker-compose.yml file | `docker-compose.yml` |

### Options

| Flag | Description |
|---|---|
| `-o, --output <PATH>` | Write output to a file instead of stdout |

### Exit Codes

| Code | Meaning |
|---|---|
| `0` | Conversion succeeded |
| `1` | File not found or YAML parse error |

### Examples

```bash
# Convert and print to stdout
ctst convert

# Convert a specific file
ctst convert docker-compose.prod.yml

# Convert and save to a .ctst file
ctst convert -o infrastructure.ctst

# Convert a specific file to a specific output
ctst convert docker-compose.yml -o app.ctst

# Pipe to ctst plan for immediate preview
ctst convert -o app.ctst && ctst plan app.ctst
```

### Example Output

Given this `docker-compose.yml`:

```yaml
services:
  api:
    image: myapp:latest
    ports:
      - "8080:80"
    environment:
      DATABASE_URL: postgres://db:5432/app
    depends_on:
      - db
  db:
    image: postgres:16
    volumes:
      - pgdata:/var/lib/postgresql/data
    environment:
      POSTGRES_PASSWORD: ${DB_PASSWORD}
    restart: unless-stopped
```

Running `ctst convert` produces:

```ctst
// Auto-generated by: ctst convert
// Source: docker-compose.yml
// Components: 2
//
// Review image sources — Docker Hub references have been converted
// to tar:// placeholders. Export images with:
//   docker save <image> -o /opt/images/<name>.tar

COMPONENT api {
    image = "tar:///opt/images/myapp.tar"
    port = 80
    // EXPOSE 8080:80
    env = {
        DATABASE_URL = "postgres://db:5432/app"
    }
}

COMPONENT db {
    image = "tar:///opt/images/postgres.tar"
    volume = "pgdata:/var/lib/postgresql/data"
    env = {
        POSTGRES_PASSWORD = "${secret.DB_PASSWORD}"
    }
    restart = "always"
}

// Dependencies (converted from depends_on).
// CONNECT auto-injects _HOST, _PORT, _CONNECTION_STRING env vars.
CONNECT api -> db
```

### Post-Conversion Steps

1. **Export Docker images** to tar archives: `docker save <image> -o /opt/images/<name>.tar`
2. **Review image URIs** — replace `tar://` placeholders with actual paths
3. **Remove duplicate env vars** — `CONNECT` auto-injects `_HOST`, `_PORT`, `_CONNECTION_STRING`
4. **Add health checks** if not present in the original compose file
5. **Test with** `ctst plan app.ctst` before deploying

---

<p align="center">
  <code>ctst</code> — Built with Rust. Designed for sovereignty.
</p>
