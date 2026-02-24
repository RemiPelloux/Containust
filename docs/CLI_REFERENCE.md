# CLI Reference

The `ctst` command is the single entry point for the Containust runtime.

## Global Options

| Flag | Description | Default |
|---|---|---|
| `--offline` | Block all outbound network access | `false` |
| `--state-file <path>` | Path to the state index file | `/var/lib/containust/state.json` |
| `--help` | Show help information | — |
| `--version` | Show version | — |

## Commands

### `ctst build`

Parse a `.ctst` file and generate container images/layers.

```bash
ctst build [OPTIONS] [FILE]
```

| Argument | Description | Default |
|---|---|---|
| `FILE` | Path to the `.ctst` composition file | `containust.ctst` |

**Example:**
```bash
ctst build infrastructure.ctst
ctst build --offline
```

### `ctst plan`

Display the planned infrastructure changes without applying them (dry run).

```bash
ctst plan [OPTIONS] [FILE]
```

| Argument | Description | Default |
|---|---|---|
| `FILE` | Path to the `.ctst` composition file | `containust.ctst` |

**Example:**
```bash
ctst plan
ctst plan production.ctst
```

### `ctst run`

Deploy the component graph defined in a `.ctst` file.

```bash
ctst run [OPTIONS] [FILE]
```

| Argument | Description | Default |
|---|---|---|
| `FILE` | Path to the `.ctst` composition file | `containust.ctst` |
| `-d, --detach` | Run in detached mode | `false` |

**Example:**
```bash
ctst run
ctst run -d production.ctst
ctst run --offline
```

### `ctst ps`

List containers with their status and resource metrics.

```bash
ctst ps [OPTIONS]
```

| Flag | Description | Default |
|---|---|---|
| `-a, --all` | Show all containers (including stopped) | `false` |
| `--tui` | Launch the interactive terminal dashboard | `false` |

**Example:**
```bash
ctst ps
ctst ps --all
ctst ps --tui
```

### `ctst exec`

Execute a command inside a running container by joining its namespaces.

```bash
ctst exec <CONTAINER> -- <COMMAND...>
```

| Argument | Description |
|---|---|
| `CONTAINER` | Container ID or name |
| `COMMAND...` | Command and arguments to execute |

**Example:**
```bash
ctst exec my-api -- /bin/sh
ctst exec db-1 -- psql -U postgres
```

### `ctst stop`

Stop containers and clean up their resources (cgroups, mounts, state).

```bash
ctst stop [OPTIONS] [CONTAINERS...]
```

| Argument | Description | Default |
|---|---|---|
| `CONTAINERS` | Container IDs/names to stop | All containers |
| `-f, --force` | Force kill without graceful shutdown | `false` |

**Example:**
```bash
ctst stop
ctst stop my-api db-1
ctst stop --force
```

### `ctst images`

Manage the local image catalog.

```bash
ctst images [OPTIONS]
```

| Flag | Description |
|---|---|
| `-l, --list` | List all local images |
| `--remove <ID>` | Remove an image by ID |

**Example:**
```bash
ctst images --list
ctst images --remove sha256:abc123...
```
