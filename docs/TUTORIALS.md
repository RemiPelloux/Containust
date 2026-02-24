# Containust Tutorials

> Nine hands-on, self-contained tutorials to take you from your first container to production-grade deployments.

**CLI binary:** `ctst`  
**Composition files:** `.ctst`  
**Rust SDK crate:** `containust-sdk`

---

## Table of Contents

1. [Hello World](#tutorial-1-hello-world)
2. [Web Server with Port Exposure](#tutorial-2-web-server-with-port-exposure)
3. [Full Stack Application (API + PostgreSQL + Redis)](#tutorial-3-full-stack-application)
4. [Custom Images from Local Sources](#tutorial-4-custom-images-from-local-sources)
5. [Reusable Templates with FROM](#tutorial-5-reusable-templates-with-from)
6. [Secrets Management](#tutorial-6-secrets-management)
7. [Health Checks and Restart Policies](#tutorial-7-health-checks-and-restart-policies)
8. [Offline / Air-Gapped Deployment](#tutorial-8-offline--air-gapped-deployment)
9. [Using the Rust SDK](#tutorial-9-using-the-rust-sdk)

---

## Tutorial 1: Hello World

### What You'll Learn

> - How to write a minimal `.ctst` composition file
> - How to run a single container with `ctst run`
> - What happens behind the scenes when Containust launches a container

### Prerequisites

- Containust installed (`ctst --version` prints a version string)
- An Alpine root filesystem available at `/opt/images/alpine` (or any `file://` path)
- Linux kernel 5.10+ with user namespaces enabled

### Steps

**1. Create the composition file**

Create a file called `hello.ctst` in your working directory:

```ctst
// hello.ctst — The simplest Containust composition.
COMPONENT hello {
    image   = "file:///opt/images/alpine"
    command = ["/bin/echo", "Hello from Containust!"]
}
```

This defines a single component named `hello` that runs the `echo` command inside an Alpine container.

**2. Preview the deployment plan**

Before running, inspect what Containust will do:

```bash
ctst plan hello.ctst
```

Expected output:

```
Plan: 1 component(s) to deploy

  + hello
      image:    file:///opt/images/alpine
      command:  /bin/echo "Hello from Containust!"
      readonly: true (default)
      network:  bridge (default)

No connections declared.
Deployment order: [hello]
```

**3. Run the container**

```bash
ctst run hello.ctst
```

Expected output:

```
[INFO] Parsing hello.ctst...
[INFO] Validating composition graph...
[INFO] Loading image: file:///opt/images/alpine
[INFO]   SHA-256: a3f2b8c...d94e1 ✓
[INFO] Creating namespaces for 'hello' (pid, mount, net, uts, ipc)
[INFO] Setting up cgroups v2 for 'hello'
[INFO] Mounting read-only rootfs via OverlayFS
[INFO] Spawning process: /bin/echo "Hello from Containust!"
Hello from Containust!
[INFO] Process exited with code 0
[INFO] Cleaning up namespaces and cgroups for 'hello'
[INFO] Done.
```

**4. Verify cleanup**

```bash
ctst ps
```

Expected output:

```
No running containers.
```

### What Happened

1. **Parse & validate** — The `.ctst` file was parsed and statically analyzed for errors.
2. **Image load** — The Alpine rootfs was loaded from disk and verified with SHA-256.
3. **Namespace creation** — Linux namespaces (PID, mount, network, UTS, IPC) were created to isolate the container.
4. **Cgroup setup** — A cgroups v2 hierarchy was configured for resource limits.
5. **OverlayFS mount** — The rootfs was mounted read-only via OverlayFS.
6. **Process spawn** — The `echo` command ran inside the isolated environment.
7. **Cleanup** — All namespaces, cgroups, and mounts were torn down. No daemon lingers.

### Summary

You ran your first container with Containust — a single `echo` command in a fully isolated Linux namespace, with no daemon process involved. The entire lifecycle was handled by a single `ctst run` invocation.

---

## Tutorial 2: Web Server with Port Exposure

### What You'll Learn

> - How to mount a host directory as a volume
> - How to expose container ports to the host
> - How to serve static files with nginx under Containust

### Prerequisites

- Containust installed
- An nginx root filesystem at `/opt/images/nginx`
- `curl` installed on the host

### Steps

**1. Create the project directory**

```bash
mkdir -p webserver/html
```

**2. Create a static HTML page**

Create `webserver/html/index.html`:

```html
<!DOCTYPE html>
<html lang="en">
<head><title>Containust Web</title></head>
<body>
  <h1>Served by Containust</h1>
  <p>This page is running inside an nginx container with zero daemon overhead.</p>
</body>
</html>
```

**3. Write the composition file**

Create `webserver/nginx.ctst`:

```ctst
// nginx.ctst — Static web server with volume mount and port exposure.
COMPONENT web {
    image    = "file:///opt/images/nginx"
    port     = 80
    memory   = "128MiB"
    volume   = "/absolute/path/to/webserver/html:/usr/share/nginx/html"
    readonly = true
    restart  = "always"
    healthcheck = {
        command  = ["curl", "-f", "http://localhost:80/"]
        interval = "15s"
        timeout  = "3s"
        retries  = 3
    }
}

EXPOSE 8080:80
```

Replace `/absolute/path/to/webserver/html` with the actual absolute path to your `html` directory.

**4. Run the web server**

```bash
ctst run webserver/nginx.ctst
```

Expected output:

```
[INFO] Parsing webserver/nginx.ctst...
[INFO] Validating composition graph...
[INFO] Loading image: file:///opt/images/nginx
[INFO]   SHA-256: b7c4e1a...82f3d ✓
[INFO] Creating namespaces for 'web'
[INFO] Mounting volume: /absolute/path/to/webserver/html -> /usr/share/nginx/html
[INFO] Mounting read-only rootfs via OverlayFS
[INFO] Exposing port 8080 -> 80
[INFO] Starting 'web'...
[INFO] Healthcheck: starting (grace period)
[INFO] Healthcheck: healthy ✓
[INFO] Container 'web' is running.
```

**5. Verify with curl**

```bash
curl http://localhost:8080
```

Expected output:

```html
<!DOCTYPE html>
<html lang="en">
<head><title>Containust Web</title></head>
<body>
  <h1>Served by Containust</h1>
  <p>This page is running inside an nginx container with zero daemon overhead.</p>
</body>
</html>
```

**6. Check container status**

```bash
ctst ps
```

Expected output:

```
NAME   IMAGE                       STATUS    HEALTH    PORTS        MEMORY
web    file:///opt/images/nginx    running   healthy   8080->80     42/128 MiB
```

**7. Stop the server**

```bash
ctst stop webserver/nginx.ctst
```

### Summary

You served a static website through nginx running inside a Containust container. The host directory was mounted as a read-only volume, and the container's port 80 was exposed to the host on port 8080 — all without a daemon.

---

## Tutorial 3: Full Stack Application

### What You'll Learn

> - How to define a multi-component stack (API + PostgreSQL + Redis)
> - How `CONNECT` controls startup order and auto-injects environment variables
> - How to plan, run, inspect, and stop a full composition

### Prerequisites

- Containust installed
- Root filesystems for your API, PostgreSQL, and Redis at `/opt/images/`
- A secret environment variable for the database password

### Steps

**1. Set up the database secret**

```bash
export CONTAINUST_SECRET_DB_PASS="super_s3cure_p@ssword"
```

**2. Write the composition file**

Create `stack.ctst`:

```ctst
// stack.ctst — Full stack: API + PostgreSQL + Redis
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
        command      = ["curl", "-f", "http://localhost:8080/healthz"]
        interval     = "10s"
        timeout      = "3s"
        retries      = 5
        start_period = "15s"
    }
}

COMPONENT db {
    image  = "file:///opt/images/postgres-16"
    port   = 5432
    memory = "512MiB"
    volume = "/data/postgres:/var/lib/postgresql/data"
    env    = {
        POSTGRES_DB       = "app"
        POSTGRES_USER     = "app_user"
        POSTGRES_PASSWORD = "${secret.db_pass}"
    }
    readonly = false
    healthcheck = {
        command  = ["pg_isready", "-U", "app_user"]
        interval = "5s"
        timeout  = "2s"
        retries  = 10
    }
}

COMPONENT cache {
    image    = "tar:///opt/images/redis-7.tar"
    port     = 6379
    memory   = "128MiB"
    readonly = true
    command  = ["redis-server", "--maxmemory", "100mb"]
    healthcheck = {
        command  = ["redis-cli", "ping"]
        interval = "5s"
        timeout  = "2s"
        retries  = 5
    }
}

CONNECT api -> db
CONNECT api -> cache

EXPOSE 8080
```

**3. Preview the deployment plan**

```bash
ctst plan stack.ctst
```

Expected output:

```
Plan: 3 component(s) to deploy

  + db
      image:  file:///opt/images/postgres-16
      port:   5432
      memory: 512 MiB

  + cache
      image:  tar:///opt/images/redis-7.tar
      port:   6379
      memory: 128 MiB

  + api (depends on: db, cache)
      image:  file:///opt/images/myapp-api
      port:   8080
      memory: 256 MiB
      auto-injected env:
        DB_HOST              = <db.host>
        DB_PORT              = 5432
        DB_CONNECTION_STRING = postgres://<db.host>:5432
        CACHE_HOST           = <cache.host>
        CACHE_PORT           = 6379
        CACHE_CONNECTION_STRING = redis://<cache.host>:6379

Deployment order: [db, cache] -> [api]
Host port exposure: 8080 -> api:8080
```

**4. Deploy the stack**

```bash
ctst run stack.ctst
```

Expected output:

```
[INFO] Starting deployment of 3 components...
[INFO] Phase 1/2: Starting independent components [db, cache]
[INFO]   db: namespaces created, image loaded, starting process...
[INFO]   cache: namespaces created, image loaded, starting process...
[INFO]   db: healthcheck healthy ✓
[INFO]   cache: healthcheck healthy ✓
[INFO] Phase 2/2: Starting dependent components [api]
[INFO]   api: injecting DB_HOST, DB_PORT, DB_CONNECTION_STRING
[INFO]   api: injecting CACHE_HOST, CACHE_PORT, CACHE_CONNECTION_STRING
[INFO]   api: namespaces created, image loaded, starting process...
[INFO]   api: healthcheck healthy ✓
[INFO] All 3 components running. Exposed: 8080 -> api:8080
```

**5. Verify the running stack**

```bash
ctst ps
```

Expected output:

```
NAME    IMAGE                            STATUS    HEALTH    PORTS        MEMORY
db      file:///opt/images/postgres-16   running   healthy   5432         120/512 MiB
cache   tar:///opt/images/redis-7.tar    running   healthy   6379         18/128 MiB
api     file:///opt/images/myapp-api     running   healthy   8080->8080   64/256 MiB
```

**6. Execute a command inside a running container**

```bash
ctst exec db -- psql -U app_user -d app -c "SELECT version();"
```

Expected output:

```
                          version
-----------------------------------------------------------
 PostgreSQL 16.2 on x86_64-pc-linux-gnu, compiled by gcc
(1 row)
```

**7. Stop everything**

```bash
ctst stop stack.ctst
```

Expected output:

```
[INFO] Stopping api...
[INFO] Stopping cache...
[INFO] Stopping db...
[INFO] Cleaning up namespaces and cgroups...
[INFO] All components stopped.
```

### Summary

You deployed a three-tier application with automatic dependency ordering. `CONNECT` ensured PostgreSQL and Redis were healthy before the API started, and it auto-injected connection information as environment variables. The entire stack ran without any daemon.

---

## Tutorial 4: Custom Images from Local Sources

### What You'll Learn

> - How to create images from local directories using `file://`
> - How to create images from tar archives using `tar://`
> - How SHA-256 verification works during the build process

### Prerequisites

- Containust installed
- Basic Linux filesystem utilities (`mkdir`, `tar`, `chmod`)

### Steps

**1. Create a minimal root filesystem**

```bash
mkdir -p myimage/rootfs/{bin,lib,etc}

# Copy a statically linked binary (example: busybox)
cp /usr/bin/busybox myimage/rootfs/bin/
chmod +x myimage/rootfs/bin/busybox

# Create symlinks for common utilities
ln -s busybox myimage/rootfs/bin/sh
ln -s busybox myimage/rootfs/bin/echo
ln -s busybox myimage/rootfs/bin/ls

# Add a minimal /etc/passwd
echo "root:x:0:0:root:/root:/bin/sh" > myimage/rootfs/etc/passwd
```

**2. Write a composition using `file://`**

Create `myimage/from-dir.ctst`:

```ctst
// from-dir.ctst — Image sourced from a local directory.
COMPONENT app {
    image   = "file:///absolute/path/to/myimage/rootfs"
    command = ["/bin/echo", "Built from a local directory!"]
}
```

Replace the path with the absolute path to your `rootfs` directory.

**3. Build and verify the directory-based image**

```bash
ctst build myimage/from-dir.ctst
```

Expected output:

```
[INFO] Parsing myimage/from-dir.ctst...
[INFO] Building image for 'app' from file:///absolute/path/to/myimage/rootfs
[INFO] Computing SHA-256 of directory tree...
[INFO]   SHA-256: e4a1c7f...b82d3 ✓
[INFO] Analyzing binary dependencies (distroless)...
[INFO]   /bin/busybox: statically linked, no shared libraries needed
[INFO] Image layer created: 1.2 MiB
[INFO] Build complete. 1 image(s) ready.
```

**4. Run the directory-based image**

```bash
ctst run myimage/from-dir.ctst
```

Expected output:

```
Built from a local directory!
```

**5. Create a tar archive from the rootfs**

```bash
cd myimage/rootfs
tar cf /opt/images/myimage.tar .
cd ../..
```

**6. Write a composition using `tar://`**

Create `myimage/from-tar.ctst`:

```ctst
// from-tar.ctst — Image sourced from a tar archive.
COMPONENT app {
    image   = "tar:///opt/images/myimage.tar"
    command = ["/bin/echo", "Built from a tar archive!"]
}
```

**7. Build and verify the tar-based image**

```bash
ctst build myimage/from-tar.ctst
```

Expected output:

```
[INFO] Parsing myimage/from-tar.ctst...
[INFO] Building image for 'app' from tar:///opt/images/myimage.tar
[INFO] Verifying archive SHA-256...
[INFO]   SHA-256: 7d2f9a1...c45e8 ✓
[INFO] Extracting tar archive...
[INFO] Image layer created: 1.2 MiB
[INFO] Build complete. 1 image(s) ready.
```

**8. List cached images**

```bash
ctst images
```

Expected output:

```
HASH                   SOURCE                               SIZE      CREATED
e4a1c7f...b82d3        file:///absolute/path/to/rootfs       1.2 MiB   2 minutes ago
7d2f9a1...c45e8        tar:///opt/images/myimage.tar         1.2 MiB   30 seconds ago
```

### Summary

You created container images from both a local directory (`file://`) and a tar archive (`tar://`). Containust verified content integrity with SHA-256 hashing and performed automatic distroless analysis on your binaries. Both protocols work fully offline.

---

## Tutorial 5: Reusable Templates with FROM

### What You'll Learn

> - How to create reusable component templates
> - How `IMPORT` and `FROM` enable template inheritance
> - How child components override and merge parent properties

### Prerequisites

- Containust installed
- Root filesystems for PostgreSQL and Redis at `/opt/images/`

### Steps

**1. Create the templates directory**

```bash
mkdir -p templates
```

**2. Create a PostgreSQL template**

Create `templates/postgres.ctst`:

```ctst
// templates/postgres.ctst — Reusable PostgreSQL template.
COMPONENT postgres {
    image    = "file:///opt/images/postgres-16"
    port     = 5432
    memory   = "256MiB"
    cpu      = "512"
    readonly = false
    restart  = "on-failure"
    env      = {
        POSTGRES_USER = "postgres"
        PGDATA        = "/var/lib/postgresql/data"
    }
    healthcheck = {
        command  = ["pg_isready", "-U", "postgres"]
        interval = "5s"
        timeout  = "2s"
        retries  = 10
    }
}
```

**3. Create a Redis template**

Create `templates/redis.ctst`:

```ctst
// templates/redis.ctst — Reusable Redis template.
COMPONENT redis {
    image    = "tar:///opt/images/redis-7.tar"
    port     = 6379
    memory   = "128MiB"
    readonly = true
    restart  = "on-failure"
    command  = ["redis-server", "--maxmemory", "100mb", "--appendonly", "yes"]
    healthcheck = {
        command  = ["redis-cli", "ping"]
        interval = "5s"
        timeout  = "2s"
        retries  = 5
    }
}
```

**4. Create a composition that uses the templates**

Create `app.ctst`:

```ctst
// app.ctst — Application using reusable templates.
IMPORT "templates/postgres.ctst" AS pg
IMPORT "templates/redis.ctst" AS redis_tmpl

COMPONENT api {
    image   = "file:///opt/images/myapp-api"
    port    = 8080
    memory  = "256MiB"
    env     = {
        DATABASE_URL = "postgres://${db.host}:${db.port}/myapp"
        REDIS_URL    = "redis://${cache.host}:${cache.port}/0"
    }
    command = ["./api-server"]
}

// Inherit everything from the postgres template, override specifics.
COMPONENT db FROM pg.postgres {
    memory = "512MiB"
    volume = "/data/app-db:/var/lib/postgresql/data"
    env    = {
        POSTGRES_DB       = "myapp"
        POSTGRES_PASSWORD = "${secret.db_pass}"
    }
}

// Inherit from the redis template, increase memory.
COMPONENT cache FROM redis_tmpl.redis {
    memory = "256MiB"
}

CONNECT api -> db
CONNECT api -> cache

EXPOSE 8080
```

**5. Preview template inheritance**

```bash
ctst plan app.ctst
```

Expected output:

```
Plan: 3 component(s) to deploy

  + db (FROM pg.postgres)
      image:    file:///opt/images/postgres-16    (inherited)
      port:     5432                              (inherited)
      memory:   512 MiB                           (overridden: was 256 MiB)
      readonly: false                             (inherited)
      restart:  on-failure                        (inherited)
      env:
        POSTGRES_USER     = "postgres"            (inherited)
        PGDATA            = "/var/lib/..."         (inherited)
        POSTGRES_DB       = "myapp"               (added)
        POSTGRES_PASSWORD = <secret:db_pass>      (added)
      volume:   /data/app-db:/var/lib/...         (added)

  + cache (FROM redis_tmpl.redis)
      image:    tar:///opt/images/redis-7.tar     (inherited)
      port:     6379                              (inherited)
      memory:   256 MiB                           (overridden: was 128 MiB)
      command:  redis-server --maxmemory 100mb... (inherited)

  + api (depends on: db, cache)
      image:    file:///opt/images/myapp-api
      port:     8080
      memory:   256 MiB

Deployment order: [db, cache] -> [api]
```

Notice how `env` maps are **merged**: the child's `POSTGRES_DB` and `POSTGRES_PASSWORD` are added alongside the parent's `POSTGRES_USER` and `PGDATA`.

**6. Deploy**

```bash
export CONTAINUST_SECRET_DB_PASS="template_demo_pass"
ctst run app.ctst
```

### Summary

You created reusable PostgreSQL and Redis templates that encapsulate best-practice defaults. The main composition imported them and selectively overrode properties. Template inheritance reduces duplication and enforces organizational standards across projects.

---

## Tutorial 6: Secrets Management

### What You'll Learn

> - How to inject secrets using `${secret.name}` interpolation
> - The two secret resolution sources: environment variables and secret files
> - How to verify that secrets are never stored in the state file
> - How to rotate secrets without rebuilding images

### Prerequisites

- Containust installed
- A PostgreSQL root filesystem at `/opt/images/postgres-16`

### Steps

**1. Set secrets via environment variables**

The naming convention is `CONTAINUST_SECRET_<NAME>` (uppercased):

```bash
export CONTAINUST_SECRET_DB_PASS="initial_p@ssword_123"
export CONTAINUST_SECRET_API_KEY="sk_live_abc123xyz789"
```

**2. Write a composition that references secrets**

Create `secrets-demo.ctst`:

```ctst
// secrets-demo.ctst — Demonstrating secret injection.
COMPONENT db {
    image    = "file:///opt/images/postgres-16"
    port     = 5432
    memory   = "256MiB"
    readonly = false
    env      = {
        POSTGRES_DB       = "secure_app"
        POSTGRES_PASSWORD = "${secret.db_pass}"
    }
    healthcheck = {
        command  = ["pg_isready", "-U", "postgres"]
        interval = "5s"
        timeout  = "2s"
        retries  = 10
    }
}

COMPONENT api {
    image   = "file:///opt/images/myapp-api"
    port    = 8080
    env     = {
        DATABASE_URL   = "postgres://${db.host}:${db.port}/secure_app"
        STRIPE_API_KEY = "${secret.api_key}"
    }
    command = ["./api-server"]
}

CONNECT api -> db

EXPOSE 8080
```

**3. Deploy with secrets**

```bash
ctst run secrets-demo.ctst
```

Expected output:

```
[INFO] Resolving secret 'db_pass'... found in environment ✓
[INFO] Resolving secret 'api_key'... found in environment ✓
[INFO] Starting db...
[INFO] Starting api...
[INFO] All components running.
```

**4. Verify secrets are NOT in the state file**

```bash
ctst ps --state-file
```

Expected output shows the state file path. Inspect it:

```bash
cat $(ctst ps --state-file-path)
```

Expected: the JSON state file contains component metadata but **no secret values**:

```json
{
  "components": {
    "db": {
      "status": "running",
      "image": "file:///opt/images/postgres-16",
      "pid": 12345,
      "ports": [5432]
    },
    "api": {
      "status": "running",
      "image": "file:///opt/images/myapp-api",
      "pid": 12346,
      "ports": [8080]
    }
  }
}
```

No `POSTGRES_PASSWORD` or `STRIPE_API_KEY` values appear anywhere in the file.

**5. Alternative: use secret files**

For environments where environment variables are not ideal, use secret files:

```bash
sudo mkdir -p /run/containust/secrets
echo "file_based_p@ssword" | sudo tee /run/containust/secrets/db_pass > /dev/null
sudo chmod 0400 /run/containust/secrets/db_pass
```

The same `${secret.db_pass}` reference in the `.ctst` file resolves from the file automatically. Environment variables take priority if both exist.

**6. Rotate a secret**

Secret rotation requires no image rebuild — just update the source and restart:

```bash
export CONTAINUST_SECRET_DB_PASS="rotated_n3w_p@ssword"
ctst stop secrets-demo.ctst
ctst run secrets-demo.ctst
```

The containers start with the new secret value injected at process creation time.

### Summary

Secrets in Containust are resolved at deploy time from environment variables or files, injected directly into container processes, and never persisted to the state file or logs. Rotation is a simple stop-and-restart cycle with no image changes required.

---

## Tutorial 7: Health Checks and Restart Policies

### What You'll Learn

> - How to configure health monitoring with the `healthcheck` property
> - How restart policies (`never`, `on-failure`, `always`) interact with health state
> - How to observe automatic restarts and health transitions

### Prerequisites

- Containust installed
- Root filesystems for nginx and a test application at `/opt/images/`

### Steps

**1. Write a composition with health checks and restart policies**

Create `health-demo.ctst`:

```ctst
// health-demo.ctst — Health checks and automatic restarts.
COMPONENT web {
    image   = "file:///opt/images/nginx"
    port    = 80
    memory  = "128MiB"
    restart = "always"
    healthcheck = {
        command      = ["curl", "-f", "http://localhost:80/"]
        interval     = "10s"
        timeout      = "3s"
        retries      = 3
        start_period = "5s"
    }
}

COMPONENT worker {
    image   = "file:///opt/images/worker"
    memory  = "64MiB"
    restart = "on-failure"
    command = ["./process-jobs", "--max-retries", "3"]
    healthcheck = {
        command  = ["pgrep", "-f", "process-jobs"]
        interval = "15s"
        timeout  = "2s"
        retries  = 2
    }
}

EXPOSE 8080:80
```

**2. Deploy and observe health state transitions**

```bash
ctst run health-demo.ctst
```

Expected output:

```
[INFO] Starting web...
[INFO]   web: health state -> starting (grace period: 5s)
[INFO]   web: health state -> healthy ✓
[INFO] Starting worker...
[INFO]   worker: health state -> starting
[INFO]   worker: health state -> healthy ✓
[INFO] All components running.
```

**3. Monitor health status**

```bash
ctst ps
```

Expected output:

```
NAME     STATUS    HEALTH     RESTART    RESTARTS   UPTIME
web      running   healthy    always     0          2m 15s
worker   running   healthy    on-failure 0          2m 14s
```

**4. Simulate a failure**

Send a signal to crash the worker process:

```bash
ctst exec worker -- kill -9 1
```

**5. Watch the automatic restart**

```bash
ctst ps
```

Expected output (shortly after the failure):

```
NAME     STATUS      HEALTH      RESTART      RESTARTS   UPTIME
web      running     healthy     always       0          5m 30s
worker   restarting  unhealthy   on-failure   1          3s
```

After a few seconds:

```bash
ctst ps
```

```
NAME     STATUS    HEALTH     RESTART      RESTARTS   UPTIME
web      running   healthy    always       0          5m 45s
worker   running   healthy    on-failure   1          15s
```

The worker was automatically restarted because its restart policy is `on-failure` and the process exited with a non-zero code.

**6. Understand the health lifecycle**

| Phase | Duration | Behavior |
|---|---|---|
| `starting` | `start_period` (5s for web) | Failures do not count toward `retries` |
| `healthy` | Ongoing | Last check passed |
| `unhealthy` | After `retries` consecutive failures | Triggers restart if policy allows |

### Summary

Health checks run inside the container at defined intervals. When combined with restart policies, Containust automatically recovers from failures. The `on-failure` policy restarts only on non-zero exits or unhealthy state, while `always` restarts unconditionally. Health state is visible in `ctst ps` output.

---

## Tutorial 8: Offline / Air-Gapped Deployment

### What You'll Learn

> - How to prepare all images as local tar archives for disconnected environments
> - How to deploy with `--offline` to guarantee zero network activity
> - The complete workflow for air-gapped / classified deployments

### Prerequisites

- Containust installed
- Access to container images (to pre-cache them before going offline)
- `tar` utility

### Steps

**1. Prepare the offline image cache**

On a machine with network access, collect all required images:

```bash
mkdir -p /opt/offline-images

# Assuming you have rootfs directories or can extract them from registries.
# Create tar archives for each component.
tar cf /opt/offline-images/myapp-v2.1.tar -C /opt/images/myapp-api .
tar cf /opt/offline-images/postgres-16.tar -C /opt/images/postgres-16 .
tar cf /opt/offline-images/redis-7.tar -C /opt/images/redis-7 .
```

**2. Verify archive integrity**

Generate SHA-256 checksums for each archive:

```bash
sha256sum /opt/offline-images/*.tar
```

Expected output:

```
a1b2c3d4e5...f6g7h8  /opt/offline-images/myapp-v2.1.tar
i9j0k1l2m3...n4o5p6  /opt/offline-images/postgres-16.tar
q7r8s9t0u1...v2w3x4  /opt/offline-images/redis-7.tar
```

Save these checksums for verification after transfer to the air-gapped environment.

**3. Transfer to the air-gapped machine**

```bash
# Copy via USB drive, secure transfer, or other approved method.
# On the air-gapped machine, verify checksums:
sha256sum -c checksums.txt
```

**4. Write the offline composition**

Create `airgap.ctst` on the air-gapped machine:

```ctst
// airgap.ctst — Fully offline deployment using local tar archives.
// Deploy with: ctst run --offline airgap.ctst
COMPONENT app {
    image   = "tar:///opt/offline-images/myapp-v2.1.tar"
    port    = 8080
    memory  = "256MiB"
    env     = {
        DATABASE_URL = "postgres://${db.host}:${db.port}/secure_app"
        REDIS_URL    = "redis://${cache.host}:${cache.port}/0"
    }
    command = ["./server"]
    network = "isolated"
}

COMPONENT db {
    image    = "tar:///opt/offline-images/postgres-16.tar"
    port     = 5432
    memory   = "512MiB"
    volume   = "/secure-data/pg:/var/lib/postgresql/data"
    readonly = false
    env      = {
        POSTGRES_DB       = "secure_app"
        POSTGRES_PASSWORD = "${secret.db_pass}"
    }
    network = "isolated"
}

COMPONENT cache {
    image    = "tar:///opt/offline-images/redis-7.tar"
    port     = 6379
    memory   = "128MiB"
    readonly = true
    command  = ["redis-server", "--maxmemory", "100mb"]
    network  = "isolated"
}

CONNECT app -> db
CONNECT app -> cache

EXPOSE 8080
```

**5. Deploy in offline mode**

```bash
export CONTAINUST_SECRET_DB_PASS="airgap_s3cure_pass"
ctst run --offline airgap.ctst
```

Expected output:

```
[INFO] Offline mode: all network egress blocked
[INFO] Parsing airgap.ctst...
[INFO] Validating composition graph...
[INFO] Loading image: tar:///opt/offline-images/postgres-16.tar
[INFO]   SHA-256: i9j0k1l2m3...n4o5p6 ✓
[INFO] Loading image: tar:///opt/offline-images/redis-7.tar
[INFO]   SHA-256: q7r8s9t0u1...v2w3x4 ✓
[INFO] Loading image: tar:///opt/offline-images/myapp-v2.1.tar
[INFO]   SHA-256: a1b2c3d4e5...f6g7h8 ✓
[INFO] Starting deployment...
[INFO]   db: started ✓
[INFO]   cache: started ✓
[INFO]   app: started ✓
[INFO] All 3 components running (offline mode).
```

**6. Verify no network activity**

The `--offline` flag ensures:

- No DNS lookups are attempted
- No outbound TCP/UDP connections are established
- Any `https://` image sources in the `.ctst` file produce a **compile error** (not a runtime error)
- The `"isolated"` network mode adds an additional layer of per-container network blocking

**7. Confirm with the plan command**

```bash
ctst plan --offline airgap.ctst
```

If any component used an `https://` source, you would see:

```
error[E0014]: remote source forbidden in offline mode
  --> airgap.ctst:3:14
   |
 3 |     image = "https://registry.example.com/app:latest"
   |              ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ remote sources are
   |              blocked when --offline is set
   |
   = help: use a local source (file:// or tar://) instead
```

### Summary

The offline workflow is: pre-cache images as tar archives, transfer to the disconnected machine, write a `.ctst` using only `tar://` sources, and deploy with `--offline`. Containust guarantees zero network activity, making it suitable for classified and air-gapped environments.

---

## Tutorial 9: Using the Rust SDK

### What You'll Learn

> - How to embed Containust in a Rust application using `containust-sdk`
> - How to create containers programmatically with `ContainerBuilder`
> - How to load `.ctst` files with `GraphResolver`
> - How to monitor container events with `EventListener`

### Prerequisites

- Rust 1.85+ installed
- Linux kernel 5.10+
- Basic familiarity with Cargo and Rust projects

### Steps

**1. Create a new Rust project**

```bash
cargo init containust-demo
cd containust-demo
```

**2. Add the containust-sdk dependency**

```bash
cargo add containust-sdk
cargo add anyhow
cargo add tracing tracing-subscriber
```

Your `Cargo.toml` dependencies section will look like:

```toml
[dependencies]
containust-sdk = "0.1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

**3. Write a simple container launcher**

Replace `src/main.rs` with:

```rust
use anyhow::Result;
use containust_sdk::builder::ContainerBuilder;
use tracing_subscriber;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let container = ContainerBuilder::new("hello-sdk")
        .image("file:///opt/images/alpine")
        .command(vec![
            "/bin/echo".into(),
            "Hello from the Rust SDK!".into(),
        ])
        .memory_limit(64 * 1024 * 1024) // 64 MiB
        .cpu_shares(512)
        .readonly(true)
        .build()?;

    let exit_code = container.run()?;
    println!("Container exited with code: {exit_code}");

    Ok(())
}
```

**4. Build and run**

```bash
cargo build --release
sudo ./target/release/containust-demo
```

Expected output:

```
Hello from the Rust SDK!
Container exited with code: 0
```

**5. Load a `.ctst` file with GraphResolver**

Create a file `demo.ctst` in the project root:

```ctst
COMPONENT web {
    image   = "file:///opt/images/nginx"
    port    = 80
    memory  = "128MiB"
}

COMPONENT api {
    image   = "file:///opt/images/myapp-api"
    port    = 8080
    memory  = "256MiB"
}

CONNECT api -> web
```

Now update `src/main.rs` to load and deploy a `.ctst` file programmatically:

```rust
use anyhow::Result;
use containust_sdk::graph::GraphResolver;
use tracing_subscriber;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let graph = GraphResolver::from_file("demo.ctst")?;

    println!("Components found: {}", graph.component_count());
    println!("Deployment order:");
    for (phase, components) in graph.deployment_phases() {
        println!(
            "  Phase {}: [{}]",
            phase,
            components.join(", ")
        );
    }

    graph.deploy()?;
    println!("All components deployed successfully.");

    graph.stop_all()?;
    println!("All components stopped.");

    Ok(())
}
```

Expected output:

```
Components found: 2
Deployment order:
  Phase 1: [web]
  Phase 2: [api]
All components deployed successfully.
All components stopped.
```

**6. Monitor events with EventListener**

Add event-driven monitoring to your application:

```rust
use anyhow::Result;
use containust_sdk::builder::ContainerBuilder;
use containust_sdk::events::{EventListener, ContainerEvent};
use std::sync::Arc;
use tracing_subscriber;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let listener = Arc::new(EventListener::new());
    let listener_clone = Arc::clone(&listener);

    std::thread::spawn(move || {
        for event in listener_clone.subscribe() {
            match event {
                ContainerEvent::Started { name, pid } => {
                    println!("[EVENT] {name} started (PID: {pid})");
                }
                ContainerEvent::HealthChanged { name, status } => {
                    println!("[EVENT] {name} health -> {status}");
                }
                ContainerEvent::Stopped { name, exit_code } => {
                    println!("[EVENT] {name} stopped (exit: {exit_code})");
                }
                ContainerEvent::Restarted { name, attempt } => {
                    println!("[EVENT] {name} restarting (attempt #{attempt})");
                }
            }
        }
    });

    let container = ContainerBuilder::new("monitored-app")
        .image("file:///opt/images/alpine")
        .command(vec!["/bin/sh".into(), "-c".into(), "sleep 5 && echo done".into()])
        .event_listener(Arc::clone(&listener))
        .build()?;

    container.run()?;
    Ok(())
}
```

Expected output:

```
[EVENT] monitored-app started (PID: 54321)
[EVENT] monitored-app health -> healthy
done
[EVENT] monitored-app stopped (exit: 0)
```

**7. Integrate with an existing application**

The SDK is designed to be embedded in larger Rust applications. Common integration patterns:

```rust
// Pattern 1: On-demand container creation in a web server handler
async fn handle_build_request(payload: BuildRequest) -> Result<Response> {
    let container = ContainerBuilder::new(&payload.job_id)
        .image(&payload.image)
        .command(payload.command.clone())
        .memory_limit(payload.memory_mib * 1024 * 1024)
        .env("BUILD_ID", &payload.job_id)
        .build()?;

    let exit_code = container.run()?;
    Ok(Response::new(exit_code))
}

// Pattern 2: Loading infrastructure from .ctst files at startup
fn init_infrastructure(config_path: &str) -> Result<GraphResolver> {
    let graph = GraphResolver::from_file(config_path)?;
    graph.deploy()?;
    Ok(graph)
}
```

### Summary

The `containust-sdk` crate lets you embed container management directly in Rust applications. `ContainerBuilder` provides a fluent API for single containers, `GraphResolver` loads and deploys full `.ctst` compositions, and `EventListener` enables reactive monitoring. Because there is no daemon, the SDK makes direct Linux syscalls — your application has full control over the container lifecycle.

---

## Next Steps

Now that you have completed all nine tutorials, explore further:

- **[Language Reference](CTST_LANG.md)** — Complete `.ctst` syntax and semantics
- **[CLI Reference](CLI_REFERENCE.md)** — All `ctst` subcommands and flags
- **[SDK Guide](SDK_GUIDE.md)** — Full Rust SDK API documentation
- **[Architecture](../ARCHITECTURE.md)** — Internal design and crate structure

---

*Built with Rust. Designed for sovereignty.*
