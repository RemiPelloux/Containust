# .ctst Language Reference

> Composition language for **Containust** — a daemon-less, sovereign container runtime written in Rust.

**Version:** 1.0  
**File extension:** `.ctst`  
**Parser:** nom 8 (crate `containust-compose`)  
**Graph engine:** petgraph 0.7

---

## 1. Introduction

The `.ctst` format is the declarative composition language used by Containust to define multi-container infrastructure. Every `.ctst` file describes a complete, deployable unit: the images to run, the resources they consume, the connections between them, and the secrets they require.

### Design Philosophy

| Principle | Rationale |
|---|---|
| **Declarative** | Describe *what* you want, not *how* to achieve it. The runtime resolves ordering, wiring, and lifecycle. |
| **LLM-friendly** | Minimal syntax, consistent structure, uppercase keywords. An LLM can generate, read, and refactor `.ctst` files without special tooling. |
| **Statically analyzable** | The parser validates the entire file — types, references, cycles, unused imports — before any container is created. |
| **Local-first** | Image sources default to local paths (`file://`, `tar://`). Remote sources require explicit opt-in. |
| **Secure by default** | Read-only root filesystems, capability dropping, and secret isolation are built into the language semantics. |

---

## 2. Quick Start

A fully annotated example showing the core constructs:

```ctst
// Import a reusable PostgreSQL template from a local file.
IMPORT "templates/postgres.ctst" AS pg

// Define an API server component.
COMPONENT api {
    image   = "file:///opt/images/myapp-api"
    port    = 8080
    memory  = "256MiB"
    cpu     = "1024"
    env     = {
        RUST_LOG     = "info"
        DATABASE_URL = "postgres://${db.host}:${db.port}/app"
    }
    command  = ["./server", "--bind", "0.0.0.0:8080"]
    readonly = true
}

// Inherit defaults from the imported template, override specifics.
COMPONENT db FROM pg {
    port   = 5432
    volume = "/data/pg:/var/lib/postgresql/data"
    env    = { POSTGRES_PASSWORD = "${secret.db_pass}" }
}

// Declare the dependency: api depends on db.
// db starts first; api receives DB_HOST, DB_PORT, DB_CONNECTION_STRING.
CONNECT api -> db
```

Save this as `stack.ctst`, then:

```bash
ctst plan stack.ctst    # preview the deployment graph
ctst build stack.ctst   # build images and layers
ctst run stack.ctst     # deploy
```

---

## 3. Syntax Fundamentals

### 3.1 Comments

Single-line comments begin with `//`. There are no multi-line comments.

```ctst
// This is a comment.
COMPONENT app {     // Inline comment after a statement.
    image = "file:///opt/images/app"
}
```

### 3.2 Identifiers

Identifiers name components, aliases, and keys. They must start with a letter and contain only ASCII letters, digits, and underscores.

```
Valid:   api, db_primary, cache01, myApp
Invalid: 1service, -name, my.component
```

### 3.3 String Literals

Strings are enclosed in double quotes. Supported escape sequences:

| Sequence | Meaning |
|---|---|
| `\"` | Literal double quote |
| `\\` | Literal backslash |
| `\n` | Newline |
| `\t` | Tab |

```ctst
env = {
    GREETING = "Hello, \"world\"!\nWelcome."
    PATH     = "C:\\data\\files"
}
```

### 3.4 Numeric Literals

Integers only. No floating-point numbers.

```ctst
port = 8080
```

### 3.5 Boolean Literals

The keywords `true` and `false` (lowercase).

```ctst
readonly = true
```

### 3.6 Block Syntax

Curly braces `{ }` delimit component bodies and map values.

```ctst
COMPONENT app {
    env = {
        KEY = "value"
    }
}
```

### 3.7 List Syntax

Square brackets `[ ]` delimit ordered collections. Elements are comma-separated.

```ctst
command = ["./server", "--port", "8080"]
ports   = [8080, 8443]
```

### 3.8 Map Syntax

Maps are key-value pairs inside `{ }`, using `=` for assignment.

```ctst
env = {
    RUST_LOG = "debug"
    APP_PORT = "8080"
}
```

### 3.9 Operators

| Operator | Usage | Meaning |
|---|---|---|
| `=` | `key = value` | Assignment within a block |
| `->` | `CONNECT a -> b` | Dependency/connection from source to target |

---

## 4. Type System

Every value in a `.ctst` file has a static type. The parser validates types at parse time.

| Type | Syntax | Example | Used by |
|---|---|---|---|
| `string` | `"quoted text"` | `"info"` | image, env values, volume, workdir, user, hostname |
| `integer` | bare number | `8080` | port, ports, cpu, healthcheck retries |
| `size` | number + suffix | `"256MiB"` | memory |
| `duration` | number + suffix | `"30s"` | healthcheck interval, timeout, start_period |
| `boolean` | `true` / `false` | `true` | readonly |
| `list` | `[a, b, c]` | `[8080, 8443]` | ports, command, entrypoint, volumes |
| `map` | `{ K = "V" }` | `{ A = "1" }` | env, healthcheck |
| `uri` | `"protocol://..."` | `"file:///opt/img"` | image |

### Size Suffixes

Sizes represent memory or storage quantities as a quoted string.

| Suffix | Base | Bytes |
|---|---|---|
| `KB` | Decimal (SI) | 1,000 |
| `MB` | Decimal (SI) | 1,000,000 |
| `GB` | Decimal (SI) | 1,000,000,000 |
| `KiB` | Binary (IEC) | 1,024 |
| `MiB` | Binary (IEC) | 1,048,576 |
| `GiB` | Binary (IEC) | 1,073,741,824 |

```ctst
memory = "512MiB"   // 536,870,912 bytes
memory = "1GB"      // 1,000,000,000 bytes
```

### Duration Suffixes

Durations represent time intervals as a quoted string.

| Suffix | Meaning |
|---|---|
| `s` | Seconds |
| `m` | Minutes |
| `h` | Hours |

```ctst
interval = "30s"
timeout  = "2m"
```

---

## 5. Reserved Keywords

The following identifiers are reserved and must not be used as component names or aliases.

| Keyword | Context |
|---|---|
| `IMPORT` | File-level import declaration |
| `AS` | Alias for an import |
| `COMPONENT` | Component definition |
| `FROM` | Template inheritance |
| `CONNECT` | Dependency declaration |
| `EXPOSE` | Host port mapping |
| `HEALTHCHECK` | Health monitoring block |
| `RESTART` | Restart policy |
| `NETWORK` | Network configuration |
| `SECRET` | Secret injection reference |
| `true` | Boolean literal |
| `false` | Boolean literal |

---

## 6. IMPORT Statement

Imports bring component templates and definitions from external `.ctst` files into the current file's scope.

### Syntax

```
IMPORT "<path_or_url>" [AS <alias>]
```

### Rules

1. Local paths are resolved relative to the directory of the importing file.
2. Remote URLs must use `https://`. Plain `http://` is rejected.
3. The `AS` keyword creates a local alias for the imported file's exports.
4. When `--offline` is set, all remote imports are forbidden and produce a compile error.
5. Imports are resolved and validated before component evaluation begins.
6. Circular imports are detected and rejected.

### Resolution Order

1. Check the path relative to the current file's directory.
2. Check the path relative to the project root (directory containing the entry `.ctst` file).
3. For `https://` URLs, fetch and cache locally. Cached copies are preferred when available.

### Examples

**Local import:**

```ctst
IMPORT "templates/redis.ctst"

COMPONENT cache FROM redis {
    port = 6379
}
```

**Aliased import:**

```ctst
IMPORT "lib/databases.ctst" AS dbs

COMPONENT primary FROM dbs.postgres {
    port = 5432
}
```

**Remote import:**

```ctst
IMPORT "https://registry.example.com/templates/nginx.ctst" AS web

COMPONENT frontend FROM web.nginx {
    port = 80
}
```

---

## 7. COMPONENT Definition

A `COMPONENT` block defines a single container: its image, resources, environment, and behavior.

### Syntax

```
COMPONENT <name> [FROM <template>] {
    <property> = <value>
    ...
}
```

### Properties

| Property | Type | Default | Description |
|---|---|---|---|
| `image` | uri | *required* | Source image URI (`file://`, `tar://`, `https://`) |
| `port` | integer | — | Single exposed port |
| `ports` | list of integers | `[]` | Multiple exposed ports |
| `memory` | size | — | Memory limit (e.g., `"256MiB"`) |
| `cpu` | string | — | CPU shares (e.g., `"1024"`) |
| `env` | map | `{}` | Environment variables injected into the container |
| `volume` | string | — | Single volume mount (`"host:container"`) |
| `volumes` | list of strings | `[]` | Multiple volume mounts |
| `command` | list of strings | — | Entrypoint command and arguments |
| `entrypoint` | list of strings | — | Override the image's default entrypoint |
| `readonly` | boolean | `true` | Read-only root filesystem |
| `workdir` | string | — | Working directory inside the container |
| `user` | string | — | User and group to run as (e.g., `"1000:1000"`) |
| `hostname` | string | component name | Container hostname |
| `restart` | string | `"never"` | Restart policy: `"never"`, `"on-failure"`, `"always"` |
| `network` | string | `"bridge"` | Network mode: `"bridge"`, `"host"`, `"none"`, or custom name |
| `healthcheck` | map | — | Health monitoring configuration (see §11) |

### Rules

1. `image` is required unless the component inherits from a template that provides one.
2. `port` and `ports` are mutually exclusive. Use one or the other.
3. `volume` and `volumes` are mutually exclusive.
4. `readonly` defaults to `true` — container root filesystems are immutable unless explicitly overridden.
5. Component names must be unique within a file. Duplicates produce a compile error.

### Examples

**Minimal component:**

```ctst
COMPONENT shell {
    image    = "file:///opt/images/alpine"
    command  = ["/bin/sh"]
    readonly = true
}
```

**Full component with all properties:**

```ctst
COMPONENT api {
    image      = "file:///opt/images/myapp"
    port       = 8080
    memory     = "512MiB"
    cpu        = "2048"
    env        = {
        RUST_LOG = "debug"
        APP_ENV  = "production"
    }
    volumes    = [
        "/data/uploads:/app/uploads",
        "/config/app.toml:/app/config.toml"
    ]
    command    = ["./server", "--workers", "4"]
    entrypoint = ["/bin/sh", "-c"]
    readonly   = false
    workdir    = "/app"
    user       = "1000:1000"
    hostname   = "api-primary"
    restart    = "on-failure"
    network    = "backend"
    healthcheck = {
        command      = ["curl", "-f", "http://localhost:8080/health"]
        interval     = "15s"
        timeout      = "3s"
        retries      = 5
        start_period = "10s"
    }
}
```

---

## 8. FROM Inheritance

Templates allow reusable component definitions. A child component inherits all properties from its parent and can override any of them.

### Syntax

```
COMPONENT <name> FROM <template_name> {
    // overrides and additions
}
```

### Rules

1. The template referenced by `FROM` must be defined in the same file or brought into scope via `IMPORT`.
2. All properties from the parent are inherited. The child's properties override matching parent properties.
3. `env` maps are **merged**: the child's keys override matching parent keys; parent keys not present in the child are preserved.
4. If the template declares `required_params`, the child must provide values for all of them.
5. Templates can inherit from other templates (chaining), but circular inheritance is rejected.

### Examples

**Inheriting from an imported template:**

```ctst
IMPORT "templates/postgres.ctst" AS pg

COMPONENT primary_db FROM pg {
    port   = 5432
    memory = "1GiB"
    env    = {
        POSTGRES_DB       = "production"
        POSTGRES_PASSWORD = "${secret.db_pass}"
    }
    volume = "/data/pg:/var/lib/postgresql/data"
}
```

**Overriding specific fields from a local template:**

```ctst
COMPONENT base_worker {
    image   = "file:///opt/images/worker"
    memory  = "128MiB"
    cpu     = "512"
    restart = "on-failure"
}

COMPONENT email_worker FROM base_worker {
    env = { QUEUE = "email" }
}

COMPONENT payment_worker FROM base_worker {
    memory = "256MiB"
    env    = { QUEUE = "payments" }
}
```

---

## 9. CONNECT Statement

`CONNECT` declares a dependency between two components. It controls deployment ordering and triggers automatic environment variable injection.

### Syntax

```
CONNECT <source> -> <target>
```

This means: **source depends on target**. The target starts first.

### Deployment Ordering

When the runtime encounters `CONNECT api -> db`, it guarantees:

1. `db` is started and healthy (if a healthcheck is defined) before `api` begins.
2. If `db` fails to start, `api` is not started.

### Auto-Injected Environment Variables

When `CONNECT api -> db` is declared, the following variables are injected into the `api` container's environment:

| Variable | Value | Example |
|---|---|---|
| `DB_HOST` | Hostname or IP of `db` | `172.17.0.2` |
| `DB_PORT` | First exposed port of `db` | `5432` |
| `DB_CONNECTION_STRING` | Protocol-aware connection string | `postgres://172.17.0.2:5432` |

The variable prefix is the target component name, uppercased. Hyphens and dots are replaced with underscores.

### Connection String Protocols

The auto-generated connection string format depends on the target's image type:

| Image contains | Protocol |
|---|---|
| `postgres` | `postgres://<host>:<port>` |
| `mysql` / `mariadb` | `mysql://<host>:<port>` |
| `redis` | `redis://<host>:<port>` |
| `mongo` | `mongodb://<host>:<port>` |
| `rabbitmq` / `amqp` | `amqp://<host>:<port>` |
| *(other)* | `http://<host>:<port>` |

### Multiple Connections

A component can depend on multiple targets. Each connection injects its own set of variables.

```ctst
CONNECT api -> db
CONNECT api -> cache
CONNECT api -> queue
```

This injects `DB_HOST`, `DB_PORT`, `DB_CONNECTION_STRING`, `CACHE_HOST`, `CACHE_PORT`, `CACHE_CONNECTION_STRING`, `QUEUE_HOST`, `QUEUE_PORT`, and `QUEUE_CONNECTION_STRING` into `api`.

### Examples

**Simple connection:**

```ctst
COMPONENT app {
    image = "file:///opt/images/app"
    port  = 3000
}

COMPONENT db {
    image = "file:///opt/images/postgres"
    port  = 5432
}

CONNECT app -> db
// app receives: DB_HOST, DB_PORT, DB_CONNECTION_STRING
```

**Multi-dependency with explicit env override:**

```ctst
COMPONENT api {
    image = "file:///opt/images/api"
    port  = 8080
    env   = {
        DATABASE_URL = "postgres://${db.host}:${db.port}/myapp"
        CACHE_URL    = "redis://${cache.host}:${cache.port}/0"
    }
}

COMPONENT db {
    image  = "file:///opt/images/postgres"
    port   = 5432
    volume = "/data/pg:/var/lib/postgresql/data"
}

COMPONENT cache {
    image = "tar:///opt/images/redis.tar"
    port  = 6379
}

CONNECT api -> db
CONNECT api -> cache
```

---

## 10. EXPOSE Statement

`EXPOSE` maps a container port to a host port, making the service accessible from outside the container network.

### Syntax

```
EXPOSE <host_port>:<container_port>
EXPOSE <port>
```

When a single port is given, it is mapped identically on both host and container.

### Difference from `port`

- `port` declares a port that is visible to other containers in the composition (internal).
- `EXPOSE` publishes a port to the host machine (external).

### Examples

**Map host port 80 to container port 8080:**

```ctst
COMPONENT web {
    image = "file:///opt/images/nginx"
    port  = 8080
}

EXPOSE 80:8080
```

**Expose on the same port:**

```ctst
COMPONENT api {
    image = "file:///opt/images/api"
    port  = 3000
}

EXPOSE 3000
```

---

## 11. HEALTHCHECK Configuration

Healthchecks define how the runtime monitors a component's readiness. They affect `CONNECT` ordering — a dependency is not considered ready until its healthcheck passes.

### Syntax

Defined as a map property inside a `COMPONENT` block:

```ctst
healthcheck = {
    command      = ["curl", "-f", "http://localhost:8080/health"]
    interval     = "30s"
    timeout      = "5s"
    retries      = 3
    start_period = "10s"
}
```

### Fields

| Field | Type | Default | Description |
|---|---|---|---|
| `command` | list of strings | *required* | Command to execute inside the container |
| `interval` | duration | `"30s"` | Time between checks |
| `timeout` | duration | `"5s"` | Maximum time a single check may run |
| `retries` | integer | `3` | Consecutive failures before marking unhealthy |
| `start_period` | duration | `"0s"` | Grace period after start before checks count |

### Health States

| State | Meaning |
|---|---|
| `starting` | Within `start_period`; failures do not count |
| `healthy` | Last `retries` checks all passed |
| `unhealthy` | `retries` consecutive failures |

### Interaction with CONNECT and RESTART

- A component connected via `CONNECT` waits until its target is `healthy` before starting.
- If `restart = "on-failure"` and the component becomes `unhealthy`, it is restarted.
- If `restart = "always"`, the component is restarted regardless of health state changes.

### Examples

**HTTP health endpoint:**

```ctst
COMPONENT api {
    image = "file:///opt/images/api"
    port  = 8080
    healthcheck = {
        command      = ["curl", "-f", "http://localhost:8080/healthz"]
        interval     = "10s"
        timeout      = "3s"
        retries      = 5
        start_period = "15s"
    }
}
```

**TCP port check:**

```ctst
COMPONENT db {
    image = "file:///opt/images/postgres"
    port  = 5432
    healthcheck = {
        command  = ["pg_isready", "-U", "postgres"]
        interval = "5s"
        timeout  = "2s"
        retries  = 10
    }
}
```

---

## 12. RESTART Policy

Controls whether and when a stopped container is automatically restarted.

### Syntax

```ctst
restart = "never" | "on-failure" | "always"
```

### Policies

| Policy | Behavior |
|---|---|
| `"never"` | Container is not restarted after exit. This is the default. |
| `"on-failure"` | Restarted only if the process exits with a non-zero code or becomes `unhealthy`. |
| `"always"` | Restarted after any exit, regardless of exit code. |

### Interaction with Healthcheck

When both `restart` and `healthcheck` are configured:

- `"on-failure"` + unhealthy → restart triggered.
- `"always"` + any exit → restart triggered.
- `"never"` + unhealthy → no restart; status reported but the container stays stopped.

### Examples

**Critical service that must always run:**

```ctst
COMPONENT proxy {
    image   = "file:///opt/images/nginx"
    port    = 80
    restart = "always"
}
```

**Worker that retries on failure:**

```ctst
COMPONENT worker {
    image   = "file:///opt/images/worker"
    restart = "on-failure"
    command = ["./process-jobs"]
}
```

---

## 13. NETWORK Configuration

Controls network isolation for a component.

### Syntax

```ctst
network = "bridge" | "host" | "none" | "<custom_name>"
```

### Modes

| Mode | Description |
|---|---|
| `"bridge"` | Default. The component gets its own network namespace with a virtual bridge. Components on the same bridge can communicate by hostname. |
| `"host"` | The component shares the host's network namespace. No isolation. Use only when performance requires it. |
| `"none"` | No network access. The container is fully isolated from all networks. |
| `"<custom_name>"` | A named virtual network. Components assigned to the same custom network can communicate. Components on different custom networks are isolated. |

### Communication Rules

- Components on the same `bridge` or custom network resolve each other by component name as hostname.
- `CONNECT` auto-wiring works across any network mode, but the target must be reachable from the source.
- `"none"` prevents all network communication, including between connected components.

### Examples

**Default bridge (implicit):**

```ctst
COMPONENT api {
    image = "file:///opt/images/api"
    port  = 8080
    // network = "bridge" is the default
}
```

**Custom network for isolation:**

```ctst
COMPONENT frontend {
    image   = "file:///opt/images/web"
    port    = 80
    network = "public"
}

COMPONENT backend {
    image   = "file:///opt/images/api"
    port    = 8080
    network = "internal"
}

COMPONENT db {
    image   = "file:///opt/images/postgres"
    port    = 5432
    network = "internal"
}

// backend and db can communicate (same network).
// frontend cannot reach db directly.
CONNECT backend -> db
```

---

## 14. SECRET Injection

Secrets are sensitive values (passwords, tokens, API keys) that must never be hardcoded in `.ctst` files or stored in the state file.

### Syntax

```ctst
env = {
    DB_PASSWORD = "${secret.db_password}"
    API_KEY     = "${secret.stripe_key}"
}
```

### Resolution Order

Secrets referenced via `${secret.<name>}` are resolved at deploy time in this order:

1. **Environment variable** on the host: the runtime checks for an environment variable named `CONTAINUST_SECRET_<NAME>` (uppercased, with dots replaced by underscores).
2. **Secret file**: the runtime reads from `/run/containust/secrets/<name>`.
3. If neither source provides a value, deployment fails with an actionable error.

### Security Guarantees

- Secrets are injected into the container's environment at process creation time.
- Secrets are **never** written to the state file (`state.json`).
- Secrets are **never** logged — the runtime scrubs secret values from all log output.
- Secret files must have restrictive permissions (`0400` or `0600`).

### Examples

**Database password from host environment:**

```bash
export CONTAINUST_SECRET_DB_PASS="s3cure_p@ss"
ctst run stack.ctst
```

```ctst
COMPONENT db {
    image = "file:///opt/images/postgres"
    port  = 5432
    env   = {
        POSTGRES_PASSWORD = "${secret.db_pass}"
    }
}
```

**API key from secret file:**

```bash
echo "sk_live_abc123" > /run/containust/secrets/stripe_key
chmod 0400 /run/containust/secrets/stripe_key
```

```ctst
COMPONENT payment {
    image = "file:///opt/images/payment-svc"
    port  = 8080
    env   = {
        STRIPE_API_KEY = "${secret.stripe_key}"
    }
}
```

---

## 15. Variable Interpolation

Variable interpolation allows dynamic values in string properties using `${}` syntax.

### Syntax

```
${<namespace>.<property>}
```

### Namespaces

| Namespace | Syntax | Description |
|---|---|---|
| Component | `${component_name.host}` | Access a sibling component's runtime properties |
| Component | `${component_name.port}` | First exposed port of the component |
| Component | `${component_name.connection_string}` | Auto-generated connection string |
| Secret | `${secret.name}` | Resolve a secret value (see §14) |
| Host env | `${env.NAME}` | Read an environment variable from the host |

### Component Properties

When referencing another component, the following properties are available:

| Property | Type | Description |
|---|---|---|
| `host` | string | Hostname or IP address assigned to the component |
| `port` | integer | First declared port of the component |
| `connection_string` | string | Protocol-aware connection URL |

### Rules

1. Interpolation is only valid inside string values (double-quoted strings).
2. Nested interpolation is not supported: `${${name}.host}` is invalid.
3. References to undefined components produce a compile error.
4. Interpolated values are resolved at deploy time, not at parse time.

### Examples

**Component property access:**

```ctst
env = {
    DATABASE_URL = "postgres://${db.host}:${db.port}/myapp"
}
```

**Secret reference:**

```ctst
env = {
    JWT_SECRET = "${secret.jwt_signing_key}"
}
```

**Host environment variable:**

```ctst
env = {
    LOG_LEVEL = "${env.RUST_LOG}"
    DEPLOY_ENV = "${env.APP_ENVIRONMENT}"
}
```

---

## 16. Image Source Protocols

Containust supports three protocols for image sources. All protocols validate content integrity with SHA-256 checksums.

### Protocols

| Protocol | Format | Description |
|---|---|---|
| `file://` | `file:///absolute/path` | Local directory containing an unpacked root filesystem |
| `tar://` | `tar:///absolute/path.tar` | Local tar archive containing a root filesystem |
| `https://` | `https://host/path:tag` | Remote registry or archive (requires network) |

### SHA-256 Validation

All protocols validate image integrity:

- **`file://`** — the runtime computes a SHA-256 hash of the directory tree and compares it against the stored manifest.
- **`tar://`** — the archive's SHA-256 hash is verified before extraction.
- **`https://`** — the downloaded content's SHA-256 hash is verified against the registry manifest.

If validation fails, the build is aborted with an error.

### Offline Mode

When `--offline` is set:

- `file://` and `tar://` work normally.
- `https://` sources produce a compile error.
- Previously cached remote images remain available from the local cache.

### Examples

**Local directory:**

```ctst
COMPONENT app {
    image = "file:///opt/images/alpine"
}
```

**Local tar archive:**

```ctst
COMPONENT cache {
    image = "tar:///opt/images/redis-7.2.tar"
}
```

**Remote registry:**

```ctst
COMPONENT proxy {
    image = "https://registry.example.com/images/nginx:1.25"
}
```

---

## 17. Static Analysis

The parser performs comprehensive validation before any container is created. All errors are reported with file location and actionable messages.

### Checks Performed

| Check | Severity | Description |
|---|---|---|
| Undefined component reference | Error | A `CONNECT` or interpolation references a component that does not exist |
| Duplicate component name | Error | Two `COMPONENT` blocks share the same name |
| Cyclic dependency | Error | The `CONNECT` graph contains a cycle (A → B → A) |
| Undefined template | Error | A `FROM` clause references a template that is not defined or imported |
| Missing required parameter | Error | A `FROM` child omits a parameter the template declares as required |
| Invalid image URI | Error | The `image` value does not match `file://`, `tar://`, or `https://` |
| Type mismatch | Error | A property value does not match its expected type (e.g., string for `port`) |
| Unused import | Warning | An `IMPORT` is declared but no component references it |
| Unreachable component | Warning | A component is defined but not referenced by any `CONNECT` or `EXPOSE` |
| Circular import | Error | File A imports B which imports A |
| Mutually exclusive properties | Error | Both `port` and `ports`, or both `volume` and `volumes`, are set |

### Example Error Output

```
error[E0001]: undefined component reference
  --> stack.ctst:15:12
   |
15 | CONNECT api -> database
   |                ^^^^^^^^ component 'database' is not defined
   |
   = help: did you mean 'db'?

warning[W0001]: unused import
  --> stack.ctst:1:1
   |
 1 | IMPORT "templates/redis.ctst" AS cache_tmpl
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ 'cache_tmpl' is imported but never used
```

---

## 18. Distroless Build Analysis

Containust can automatically produce minimal container images by analyzing which files a binary actually needs at runtime.

### What It Is

A distroless build strips the container image down to only the executable and its required shared libraries — no shell, no package manager, no unnecessary files.

### How It Works

1. During `ctst build`, the analyzer inspects the target binary using an internal `ldd` equivalent.
2. It resolves all dynamically linked shared libraries (`.so` files).
3. It copies only the binary, its required libraries, and declared static assets into the final image layer.
4. The resulting image contains the absolute minimum needed to run the process.

### When It Runs

Distroless analysis runs automatically during `ctst build` for components whose image source is a `file://` directory containing a binary. It can be skipped with `--no-distroless`.

### Benefits

- Attack surface reduction: no shell means no shell-based exploits.
- Smaller images: typical reduction of 60–90% compared to full base images.
- Faster deployment: less data to transfer and mount.

---

## 19. Complete Examples

### Example 1: Hello World

The simplest possible `.ctst` file — a single container running a shell.

```ctst
COMPONENT hello {
    image   = "file:///opt/images/alpine"
    command = ["/bin/echo", "Hello from Containust!"]
}
```

### Example 2: Web Server with Volume

A static file server with a mounted content directory.

```ctst
COMPONENT web {
    image    = "file:///opt/images/nginx"
    port     = 80
    memory   = "128MiB"
    volume   = "/srv/www:/usr/share/nginx/html"
    readonly = true
    restart  = "always"
    healthcheck = {
        command  = ["curl", "-f", "http://localhost:80/"]
        interval = "15s"
        timeout  = "3s"
        retries  = 3
    }
}

EXPOSE 80
```

### Example 3: Full Stack — API + Database + Cache

A three-tier architecture with dependency wiring.

```ctst
IMPORT "templates/postgres.ctst" AS pg

COMPONENT api {
    image   = "file:///opt/images/myapp-api"
    port    = 8080
    memory  = "256MiB"
    cpu     = "1024"
    env     = {
        RUST_LOG     = "info"
        DATABASE_URL = "postgres://${db.host}:${db.port}/app"
        REDIS_URL    = "redis://${cache.host}:${cache.port}/0"
    }
    command  = ["./api-server", "--bind", "0.0.0.0:8080"]
    readonly = true
    restart  = "on-failure"
    healthcheck = {
        command  = ["curl", "-f", "http://localhost:8080/healthz"]
        interval = "10s"
        timeout  = "3s"
        retries  = 5
    }
}

COMPONENT db FROM pg {
    port   = 5432
    memory = "512MiB"
    volume = "/data/postgres:/var/lib/postgresql/data"
    env    = {
        POSTGRES_DB       = "app"
        POSTGRES_USER     = "app_user"
        POSTGRES_PASSWORD = "${secret.db_password}"
    }
}

COMPONENT cache {
    image    = "tar:///opt/images/redis-7.tar"
    port     = 6379
    memory   = "128MiB"
    readonly = true
    command  = ["redis-server", "--maxmemory", "100mb"]
}

CONNECT api -> db
CONNECT api -> cache

EXPOSE 8080
```

### Example 4: Microservices with Templates, Secrets, and Healthchecks

A multi-service architecture demonstrating template reuse, secrets, and policies.

```ctst
IMPORT "templates/postgres.ctst" AS pg
IMPORT "templates/redis.ctst" AS redis_tmpl

// Shared base for all microservices.
COMPONENT service_base {
    image   = "file:///opt/images/platform-base"
    memory  = "128MiB"
    cpu     = "512"
    restart = "on-failure"
    env     = {
        LOG_LEVEL  = "${env.LOG_LEVEL}"
        DEPLOY_ENV = "production"
    }
    healthcheck = {
        command  = ["curl", "-f", "http://localhost:8080/healthz"]
        interval = "10s"
        timeout  = "3s"
        retries  = 5
    }
}

COMPONENT gateway FROM service_base {
    image   = "file:///opt/images/gateway"
    port    = 443
    memory  = "256MiB"
    restart = "always"
    env     = {
        JWT_SECRET = "${secret.jwt_key}"
        UPSTREAM   = "http://${user_svc.host}:${user_svc.port}"
    }
}

COMPONENT user_svc FROM service_base {
    image = "file:///opt/images/user-service"
    port  = 8081
    env   = {
        DATABASE_URL = "postgres://${user_db.host}:${user_db.port}/users"
    }
}

COMPONENT order_svc FROM service_base {
    image = "file:///opt/images/order-service"
    port  = 8082
    env   = {
        DATABASE_URL = "postgres://${order_db.host}:${order_db.port}/orders"
        CACHE_URL    = "redis://${cache.host}:${cache.port}/1"
    }
}

COMPONENT user_db FROM pg {
    port   = 5432
    memory = "512MiB"
    volume = "/data/user-db:/var/lib/postgresql/data"
    env    = { POSTGRES_PASSWORD = "${secret.user_db_pass}" }
}

COMPONENT order_db FROM pg {
    port   = 5433
    memory = "512MiB"
    volume = "/data/order-db:/var/lib/postgresql/data"
    env    = { POSTGRES_PASSWORD = "${secret.order_db_pass}" }
}

COMPONENT cache FROM redis_tmpl {
    port   = 6379
    memory = "256MiB"
}

CONNECT gateway   -> user_svc
CONNECT gateway   -> order_svc
CONNECT user_svc  -> user_db
CONNECT order_svc -> order_db
CONNECT order_svc -> cache

EXPOSE 443
```

### Example 5: Air-Gapped Deployment

A stack designed for environments with no internet access, using only local tar archives.

```ctst
// All images sourced from local tar archives.
// Safe for air-gapped / classified environments.
// Deploy with: ctst run --offline airgap.ctst

COMPONENT app {
    image   = "tar:///opt/offline-images/myapp-v2.1.tar"
    port    = 8080
    memory  = "256MiB"
    env     = {
        DATABASE_URL = "postgres://${db.host}:${db.port}/secure_app"
    }
    command = ["./server"]
    network = "isolated"
}

COMPONENT db {
    image   = "tar:///opt/offline-images/postgres-16.tar"
    port    = 5432
    memory  = "512MiB"
    volume  = "/secure-data/pg:/var/lib/postgresql/data"
    env     = {
        POSTGRES_PASSWORD = "${secret.db_pass}"
    }
    network = "isolated"
}

CONNECT app -> db

EXPOSE 8080
```

---

## 20. Docker Compose Comparison

A quick reference for developers migrating from Docker Compose to `.ctst`.

| Concept | Docker Compose (`compose.yml`) | Containust (`.ctst`) |
|---|---|---|
| **File format** | YAML | Custom declarative (`.ctst`) |
| **Service definition** | `services: app:` | `COMPONENT app { }` |
| **Image** | `image: nginx:1.25` | `image = "https://registry.example.com/nginx:1.25"` |
| **Local image** | `build: ./app` | `image = "file:///opt/images/app"` |
| **Ports** | `ports: ["8080:80"]` | `port = 80` + `EXPOSE 8080:80` |
| **Volumes** | `volumes: ["./data:/data"]` | `volume = "./data:/data"` |
| **Environment** | `environment: { K: "V" }` | `env = { K = "V" }` |
| **Dependencies** | `depends_on: [db]` | `CONNECT app -> db` |
| **Auto-wiring** | *(manual)* | Automatic `_HOST`, `_PORT`, `_CONNECTION_STRING` injection |
| **Networks** | `networks: [backend]` | `network = "backend"` |
| **Restart** | `restart: unless-stopped` | `restart = "always"` |
| **Healthcheck** | `healthcheck: { test: ... }` | `healthcheck = { command = [...] }` |
| **Secrets** | `secrets:` + `docker secret` | `${secret.name}` interpolation |
| **Templating** | *(not native)* | `COMPONENT x FROM template { }` |
| **Imports** | *(not native)* | `IMPORT "file.ctst" AS alias` |
| **Offline mode** | *(not native)* | `ctst run --offline` |
| **Static analysis** | *(limited)* | Full type checking, cycle detection, unused import warnings |
| **Distroless builds** | *(manual multi-stage)* | Automatic binary dependency analysis |
| **Daemon** | Requires `dockerd` | **No daemon** — direct syscalls |

### Side-by-Side Example

**Docker Compose:**

```yaml
services:
  api:
    image: myapp:latest
    ports:
      - "8080:8080"
    environment:
      DATABASE_URL: "postgres://db:5432/app"
    depends_on:
      - db
    restart: on-failure
  db:
    image: postgres:16
    volumes:
      - pg_data:/var/lib/postgresql/data
    environment:
      POSTGRES_PASSWORD: secret
volumes:
  pg_data:
```

**Containust equivalent:**

```ctst
COMPONENT api {
    image   = "file:///opt/images/myapp"
    port    = 8080
    env     = {
        DATABASE_URL = "postgres://${db.host}:${db.port}/app"
    }
    restart = "on-failure"
}

COMPONENT db {
    image  = "file:///opt/images/postgres-16"
    port   = 5432
    volume = "/data/pg:/var/lib/postgresql/data"
    env    = {
        POSTGRES_PASSWORD = "${secret.db_password}"
    }
}

CONNECT api -> db
EXPOSE 8080
```

---

*Built with Rust. Designed for sovereignty.*
