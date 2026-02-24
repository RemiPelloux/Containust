# Migrating from Docker Compose to Containust

> A practical guide for teams moving multi-container workloads from Docker Compose to the Containust daemon-less runtime and its `.ctst` composition language.

---

## 1. Philosophy Differences

Before converting files, understand *why* the two systems diverge:

| Aspect | Docker Compose | Containust |
|---|---|---|
| **Daemon** | Requires `dockerd` running as root | No daemon — direct Linux syscalls, state file |
| **Paradigm** | Imperative YAML with implicit behavior | Declarative `.ctst` with static analysis |
| **Image source** | Online-first (`docker pull` from Docker Hub) | Local-first (`file://`, `tar://`); remote requires opt-in |
| **Security posture** | Writable rootfs, all capabilities by default | Read-only rootfs, zero capabilities by default |
| **Composition** | Flat YAML, no native templating | `IMPORT` / `FROM` template inheritance |
| **Dependency wiring** | Manual env vars + `depends_on` ordering | `CONNECT` with auto-injected `_HOST`, `_PORT`, `_CONNECTION_STRING` |
| **Offline / air-gap** | Not supported natively | First-class `--offline` flag blocks all egress |
| **Observability** | External tools (cAdvisor, Prometheus exporters) | Built-in eBPF syscall/file/network tracing |
| **Secrets** | Docker secrets API or env vars | `${secret.name}` interpolation with host env / secret files |
| **Validation** | Limited (`docker compose config`) | Full type checking, cycle detection, unused import warnings |

---

## 2. Syntax Comparison

Use this table as a quick-reference when converting each line of your `docker-compose.yml`.

| Docker Compose | Containust `.ctst` | Notes |
|---|---|---|
| `version: "3.8"` | *(not needed)* | Containust has no version field; the parser is always current |
| `services:` | `COMPONENT name { }` | Each service becomes a `COMPONENT` block |
| `image: nginx:1.25` | `image = "tar:///opt/images/nginx.tar"` | Local sources preferred; use `file://` or `tar://` |
| `build: ./app` | `ctst build` + `image = "file:///opt/images/app"` | Build the image first, then reference it locally |
| `ports: ["8080:80"]` | `port = 80` + `EXPOSE 8080:80` | `port` is internal; `EXPOSE` publishes to the host |
| `ports: ["8080:8080", "8443:8443"]` | `ports = [8080, 8443]` | Use `ports` (list) for multiple ports |
| `volumes: ["./data:/data"]` | `volume = "./data:/data"` | Single volume mount |
| `volumes:` (multiple) | `volumes = ["/a:/b", "/c:/d"]` | List syntax for multiple mounts |
| `environment: { K: "V" }` | `env = { K = "V" }` | Map syntax with `=` instead of `:` |
| `env_file: .env` | `${secret.name}` or `${env.NAME}` | Use secret interpolation or host env references |
| `depends_on: [db]` | `CONNECT app -> db` | Declares dependency *and* auto-injects env vars |
| `networks: [backend]` | `network = "backend"` | Single network assignment per component |
| `restart: unless-stopped` | `restart = "always"` | Policies: `"never"`, `"on-failure"`, `"always"` |
| `healthcheck: { test: ... }` | `healthcheck = { command = [...] }` | Same fields: `interval`, `timeout`, `retries`, `start_period` |
| `command: ["./run"]` | `command = ["./run"]` | Identical list syntax |
| `entrypoint: ["/bin/sh"]` | `entrypoint = ["/bin/sh"]` | Identical list syntax |
| `working_dir: /app` | `workdir = "/app"` | Shortened key name |
| `user: "1000:1000"` | `user = "1000:1000"` | Identical format |
| `hostname: my-host` | `hostname = "my-host"` | Defaults to component name if omitted |
| `mem_limit: 512m` | `memory = "512MiB"` | Explicit IEC/SI suffixes (`MiB`, `GiB`, `MB`, `GB`) |
| `cpus: 2.0` | `cpu = "2048"` | CPU shares (integer string), not fractional cores |
| `read_only: true` | `readonly = true` | **Default is `true`** in Containust — opt out with `false` |
| `secrets:` | `${secret.name}` | Resolved from `CONTAINUST_SECRET_*` env vars or `/run/containust/secrets/` |
| `extends: file: ...` | `COMPONENT x FROM template { }` | Native template inheritance with `IMPORT` + `FROM` |
| `docker compose up` | `ctst run stack.ctst` | Deploy the component graph |
| `docker compose down` | `ctst stop stack.ctst` | Stop and clean up |
| `docker compose ps` | `ctst ps` | List containers with metrics |
| `docker exec -it app sh` | `ctst exec app sh` | Execute inside a running container |
| `docker compose build` | `ctst build stack.ctst` | Build images and layers |
| `docker compose config` | `ctst plan stack.ctst` | Validate and preview the deployment graph |

---

## 3. Side-by-Side Example

### Docker Compose (`docker-compose.yml`)

```yaml
services:
  api:
    image: myapp-api:latest
    ports:
      - "8080:8080"
    environment:
      DATABASE_URL: "postgres://db:5432/app"
      REDIS_URL: "redis://cache:6379/0"
      JWT_SECRET: "${JWT_SECRET}"
    depends_on:
      - db
      - cache
    restart: on-failure
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/healthz"]
      interval: 10s
      timeout: 3s
      retries: 5

  db:
    image: postgres:16
    volumes:
      - pg_data:/var/lib/postgresql/data
    environment:
      POSTGRES_DB: app
      POSTGRES_PASSWORD: "${DB_PASSWORD}"

  cache:
    image: redis:7-alpine
    command: ["redis-server", "--maxmemory", "100mb"]

volumes:
  pg_data:
```

### Containust (`stack.ctst`)

```ctst
COMPONENT api {
    image   = "file:///opt/images/myapp-api"
    port    = 8080
    memory  = "256MiB"
    env     = {
        DATABASE_URL = "postgres://${db.host}:${db.port}/app"
        REDIS_URL    = "redis://${cache.host}:${cache.port}/0"
        JWT_SECRET   = "${secret.jwt_secret}"
    }
    restart = "on-failure"
    healthcheck = {
        command  = ["curl", "-f", "http://localhost:8080/healthz"]
        interval = "10s"
        timeout  = "3s"
        retries  = 5
    }
}

COMPONENT db {
    image  = "file:///opt/images/postgres-16"
    port   = 5432
    volume = "/data/pg:/var/lib/postgresql/data"
    env    = {
        POSTGRES_DB       = "app"
        POSTGRES_PASSWORD = "${secret.db_password}"
    }
}

COMPONENT cache {
    image   = "tar:///opt/images/redis-7.tar"
    port    = 6379
    memory  = "128MiB"
    command = ["redis-server", "--maxmemory", "100mb"]
}

CONNECT api -> db
CONNECT api -> cache

EXPOSE 8080
```

### Key Differences

1. **No `version:` field** — Containust files have no schema version.
2. **`services:` becomes `COMPONENT`** — uppercase keyword, curly-brace block.
3. **Images are local** — `file://` directory or `tar://` archive instead of Docker Hub tags.
4. **Named volumes become explicit paths** — `/data/pg:...` instead of `pg_data:`.
5. **Secrets use interpolation** — `${secret.db_password}` resolved from host env or secret file.
6. **Dependencies are connections** — `CONNECT api -> db` replaces `depends_on` and auto-injects `DB_HOST`, `DB_PORT`, `DB_CONNECTION_STRING` into `api`.
7. **`EXPOSE` is separate** — host port mapping is a standalone statement, not part of the component.

---

## 4. What Containust Adds

Features that have no Docker Compose equivalent:

| Feature | Description |
|---|---|
| **Auto-wired connection env vars** | `CONNECT api -> db` injects `DB_HOST`, `DB_PORT`, `DB_CONNECTION_STRING` automatically |
| **Template inheritance (`FROM`)** | `COMPONENT worker FROM base_worker { }` — reuse and override base definitions |
| **`IMPORT` composition** | `IMPORT "templates/postgres.ctst" AS pg` — modular, reusable infrastructure files |
| **Distroless auto-build** | `ctst build` analyzes binaries and strips images to only the executable + shared libs |
| **eBPF observability** | Built-in syscall tracing, file access monitoring, network socket tracking per container |
| **Offline / air-gap mode** | `ctst run --offline` blocks all outbound network — ideal for classified environments |
| **No daemon** | No `dockerd`, no persistent root process, no socket — direct Linux syscalls |
| **SHA-256 content verification** | All image sources validated with cryptographic hashes before use |
| **Read-only rootfs by default** | `readonly = true` is the default; Docker defaults to writable |
| **Static analysis** | Full type checking, cycle detection, undefined reference errors, unused import warnings |
| **Variable interpolation** | `${db.host}`, `${secret.name}`, `${env.VAR}` — first-class cross-component references |

---

## 5. Migration Checklist

Follow these steps to convert a Docker Compose project:

1. **Inventory services** — List every service in `docker-compose.yml`. Each becomes a `COMPONENT`.
2. **Export Docker images** — `docker save myapp:latest -o /opt/images/myapp.tar` for each image.
3. **Create the `.ctst` file** — One `COMPONENT` block per service with `image = "tar:///opt/images/<name>.tar"`.
4. **Convert dependencies** — Replace `depends_on: [db]` with `CONNECT app -> db`.
5. **Replace secrets and env files** — Use `${secret.name}` interpolation; set `CONTAINUST_SECRET_*` env vars on the host.
6. **Convert volumes** — Replace named volumes (`pg_data:`) with explicit host paths (`/data/pg:/var/lib/postgresql/data`).
7. **Replace remote images** — Use `file://` or `tar://` local sources instead of Docker Hub tags.
8. **Validate** — Run `ctst plan stack.ctst` to preview the deployment graph and catch errors.
9. **Deploy** — Run `ctst run stack.ctst`.

---

## 6. Common Gotchas

Things that catch Docker users by surprise:

| Gotcha | Explanation |
|---|---|
| **No background daemon** | There is no equivalent of `dockerd`. Containers are managed through a state file and direct syscalls. Use `ctst run -d` for detached mode. |
| **No named volumes** | Docker's named volumes (`pg_data:`) do not exist. Always use explicit host paths (`/data/pg:/var/lib/postgresql/data`). |
| **No implicit pull** | Images are not pulled from Docker Hub automatically. You must provide local sources via `file://` or `tar://`, or explicitly opt in to `https://`. |
| **Read-only rootfs by default** | Containers start with `readonly = true`. If your app writes to the filesystem (logs, temp files), set `readonly = false` or add a writable `volume`. |
| **Bridge is the only default network** | Custom network drivers (overlay, macvlan) are not supported. Use `"bridge"`, `"host"`, `"none"`, or named bridge networks. |
| **CONNECT auto-injects env vars** | When you write `CONNECT api -> db`, the runtime injects `DB_HOST`, `DB_PORT`, and `DB_CONNECTION_STRING` into `api`. Do not duplicate these manually in `env`. |
| **CPU is shares, not cores** | `cpu = "2048"` means 2048 CPU shares (like Docker's `--cpu-shares`), not 2.0 fractional cores. |
| **`port` vs `EXPOSE`** | `port` declares an internal port visible to other components. `EXPOSE` publishes to the host. You typically need both. |
| **No `docker-compose.override.yml`** | Use `IMPORT` and `FROM` inheritance instead of override files. |

---

## 7. FAQ

### Can I use my existing Docker images?

Yes. Export them with `docker save` and reference the tar archive:

```bash
docker save nginx:1.25 -o /opt/images/nginx.tar
```

```ctst
COMPONENT web {
    image = "tar:///opt/images/nginx.tar"
    port  = 80
}
```

### Can I run Containust alongside Docker?

Yes. Containust and Docker use separate state and do not interfere with each other. They both use Linux namespaces and cgroups, but maintain independent tracking. You can migrate services incrementally.

### How do I handle multi-stage builds?

Containust does not replicate Docker's multi-stage `Dockerfile` builds. Instead:

1. Build your binary with your existing toolchain (Cargo, Go, etc.).
2. Place the binary in a directory.
3. Run `ctst build` — distroless analysis automatically detects shared library dependencies and produces a minimal image containing only the binary and its required `.so` files.

```bash
cargo build --release --target x86_64-unknown-linux-musl
cp target/release/myapp /opt/images/myapp/
ctst build stack.ctst
```

### What about Docker Swarm or Kubernetes?

Containust is a **single-node runtime**. It does not provide orchestration across multiple machines. It replaces `docker compose` for local or single-server deployments — not Swarm or Kubernetes. For multi-node orchestration, continue using Kubernetes or evaluate purpose-built alternatives.

### Can I convert `docker-compose.yml` automatically?

Not yet. Conversion is manual. A `ctst convert` command is on the roadmap, but the semantic differences (local-first images, secret interpolation, connection auto-wiring) mean a fully automatic 1:1 translation is not always possible. This guide covers every mapping you need.

---

*Built with Rust. Designed for sovereignty.*
