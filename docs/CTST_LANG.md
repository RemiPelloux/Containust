# .ctst Language Specification

The `.ctst` format is a declarative composition language for defining multi-container infrastructure. It is designed to be human-readable, LLM-friendly, and statically analyzable.

## File Extension

All composition files use the `.ctst` extension.

## Syntax

### Comments

```ctst
// Single-line comment
```

### IMPORT

Import components or templates from other files:

```ctst
IMPORT "path/to/file.ctst"
IMPORT "path/to/file.ctst" AS alias
IMPORT "https://registry.example.com/templates/postgres.ctst" AS pg
```

Rules:
- Local paths are relative to the importing file.
- Remote imports require explicit opt-in (forbidden in `--offline` mode).
- Imports are resolved before component evaluation.

### COMPONENT

Define a container component:

```ctst
COMPONENT <name> {
    image = "<source_uri>"
    port = <number>
    memory = "<size>"
    cpu = "<shares>"
    env = {
        KEY = "value"
        KEY2 = "${other_component.variable}"
    }
    volume = "<host_path>:<container_path>"
    command = ["<binary>", "<arg1>", "<arg2>"]
    readonly = true | false
}
```

#### FROM Inheritance

Extend an imported template:

```ctst
COMPONENT mydb FROM db_template {
    port = 5432
    env = {
        POSTGRES_PASSWORD = "${secret.db_pass}"
    }
}
```

### CONNECT

Declare a dependency between two components:

```ctst
CONNECT <source> -> <target>
```

Effects:
- `target` is started before `source`.
- Connection environment variables are automatically injected into `source`:
  - `<TARGET>_HOST` — hostname/IP of the target.
  - `<TARGET>_PORT` — exposed port of the target.
  - `<TARGET>_CONNECTION_STRING` — formatted connection string (protocol-aware).

### Variable Interpolation

Use `${}` syntax to reference other component properties:

```ctst
env = {
    DATABASE_URL = "postgres://${db.host}:${db.port}/mydb"
}
```

## Size Notation

Memory and storage sizes support human-readable suffixes:

| Suffix | Meaning |
|---|---|
| `KB` | Kilobytes |
| `MB` | Megabytes |
| `GB` | Gigabytes |
| `KiB` | Kibibytes |
| `MiB` | Mebibytes |
| `GiB` | Gibibytes |

## Static Analysis

The parser performs the following checks:
- All referenced components are defined.
- No duplicate component names.
- No cyclic dependencies in CONNECT graph.
- All required parameters are provided when using FROM.
- Image sources are valid URIs.

## Complete Example

```ctst
IMPORT "templates/postgres.ctst" AS pg_template

COMPONENT api {
    image = "file:///opt/images/myapp-api"
    port = 8080
    memory = "256MiB"
    cpu = "1024"
    env = {
        RUST_LOG = "info"
        DATABASE_URL = "postgres://${db.host}:${db.port}/app"
    }
    command = ["./api-server", "--bind", "0.0.0.0:8080"]
    readonly = true
}

COMPONENT db FROM pg_template {
    port = 5432
    memory = "512MiB"
    volume = "/data/postgres:/var/lib/postgresql/data"
    env = {
        POSTGRES_DB = "app"
        POSTGRES_USER = "app_user"
    }
}

COMPONENT redis {
    image = "tar:///opt/images/redis-7.tar"
    port = 6379
    memory = "128MiB"
    readonly = true
}

CONNECT api -> db
CONNECT api -> redis
```
