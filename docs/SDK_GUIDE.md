# Containust SDK Reference

**Crate:** `containust-sdk` v0.1.0
**Rust Edition:** 2024 | **MSRV:** 1.85.0
**License:** MIT / Apache-2.0

The Containust SDK provides programmatic container management for Rust applications. It exposes three primary entry points — `ContainerBuilder`, `GraphResolver`, and `EventListener` — that wrap the engine-layer crates (`containust-runtime`, `containust-image`, `containust-compose`) behind a stable, ergonomic API surface.

Use the SDK when you need to embed container lifecycle operations directly in a Rust program without shelling out to the `ctst` CLI.

---

## Table of Contents

1. [Installation](#installation)
2. [Quick Start](#quick-start)
3. [ContainerBuilder](#containerbuilder)
4. [GraphResolver](#graphresolver)
5. [EventListener](#eventlistener)
6. [Domain Types](#domain-types)
7. [Configuration](#configuration)
8. [Error Handling](#error-handling)
9. [Patterns and Best Practices](#patterns-and-best-practices)
10. [Feature Flags](#feature-flags)
11. [Full Working Examples](#full-working-examples)

---

## Installation

Add the SDK to your `Cargo.toml`:

```toml
[dependencies]
containust-sdk = { git = "https://github.com/RemiPelloux/Containust.git" }

# Required peer dependency for error types and domain primitives
containust-common = { git = "https://github.com/RemiPelloux/Containust.git" }

# For ergonomic error handling in binaries
anyhow = "1"
```

To enable eBPF-based observability in event streams:

```toml
[dependencies]
containust-sdk = { git = "https://github.com/RemiPelloux/Containust.git", features = ["ebpf"] }
```

### Feature Flags Overview

| Feature   | Description                                        | Default |
|-----------|----------------------------------------------------|---------|
| (default) | Core SDK: builder, graph resolver, event listener  | On      |
| `ebpf`    | eBPF-powered syscall/file/network monitoring       | Off     |

---

## Quick Start

A minimal example that configures a container, builds it, and inspects its state:

```rust
use containust_sdk::builder::ContainerBuilder;
use containust_common::types::ContainerState;

fn main() -> anyhow::Result<()> {
    let container = ContainerBuilder::new("hello-containust")
        .image("file:///opt/images/alpine")
        .command(vec!["/bin/sh".into(), "-c".into(), "echo hello".into()])
        .env("RUST_LOG", "info")
        .memory_limit(64 * 1024 * 1024) // 64 MiB
        .cpu_shares(512)
        .readonly_rootfs(true)
        .build()?;

    println!("Container ID: {}", container.id);
    println!("State: {}", ContainerState::Created);
    println!("Memory limit: {:?}", container.limits.memory_bytes);
    println!("CPU shares: {:?}", container.limits.cpu_shares);

    Ok(())
}
```

---

## ContainerBuilder

`containust_sdk::builder::ContainerBuilder` — Fluent API for configuring a container before launch.

### Method Reference

| Method                 | Signature                                                    | Description                              |
|------------------------|--------------------------------------------------------------|------------------------------------------|
| `new`                  | `fn new(name: impl Into<String>) -> Self`                    | Create a builder with the given name     |
| `image`                | `fn image(self, uri: impl Into<String>) -> Self`             | Set the image source URI                 |
| `command`              | `fn command(self, cmd: Vec<String>) -> Self`                 | Set the entrypoint command               |
| `env`                  | `fn env(self, key: impl Into<String>, val: impl Into<String>) -> Self` | Add an environment variable   |
| `memory_limit`         | `const fn memory_limit(self, bytes: u64) -> Self`            | Set memory limit in bytes                |
| `cpu_shares`           | `const fn cpu_shares(self, shares: u64) -> Self`             | Set relative CPU weight                  |
| `readonly_rootfs`      | `const fn readonly_rootfs(self, readonly: bool) -> Self`     | Control root filesystem mutability       |
| `build`                | `fn build(self) -> Result<Container>`                        | Validate and produce a `Container`       |

### Detailed Method Documentation

#### `ContainerBuilder::new`

```rust
pub fn new(name: impl Into<String>) -> Self
```

Creates a new builder with the given container name. The name is used as the `ContainerId`. All fields start at safe defaults: no image, empty command, empty environment, no resource limits, and **read-only rootfs enabled**.

```rust
use containust_sdk::builder::ContainerBuilder;

let builder = ContainerBuilder::new("web-frontend");
```

#### `ContainerBuilder::image`

```rust
pub fn image(mut self, uri: impl Into<String>) -> Self
```

Sets the image source URI. Containust supports `file://` and `tar://` protocols for local-first operation. This field is **required** — calling `build()` without setting an image returns `ContainustError::Config`.

```rust
let builder = ContainerBuilder::new("api")
    .image("file:///opt/images/myapp");
```

#### `ContainerBuilder::command`

```rust
pub fn command(mut self, cmd: Vec<String>) -> Self
```

Sets the command (entrypoint) to execute inside the container. Pass each argument as a separate element.

```rust
let builder = ContainerBuilder::new("worker")
    .image("file:///opt/images/worker")
    .command(vec!["./worker".into(), "--threads".into(), "4".into()]);
```

#### `ContainerBuilder::env`

```rust
pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self
```

Adds a single environment variable. Call multiple times to add several variables. Variables are passed to the container process at startup.

```rust
let builder = ContainerBuilder::new("api")
    .image("file:///opt/images/api")
    .env("DATABASE_URL", "postgres://db:5432/app")
    .env("RUST_LOG", "info")
    .env("PORT", "8080");
```

#### `ContainerBuilder::memory_limit`

```rust
pub const fn memory_limit(mut self, bytes: u64) -> Self
```

Sets the cgroup v2 memory limit in bytes. The container process is OOM-killed if it exceeds this value.

| Human-readable | Bytes                    |
|----------------|--------------------------|
| 64 MiB         | `64 * 1024 * 1024`       |
| 256 MiB        | `256 * 1024 * 1024`      |
| 1 GiB          | `1024 * 1024 * 1024`     |
| 4 GiB          | `4 * 1024 * 1024 * 1024` |

```rust
let builder = ContainerBuilder::new("cache")
    .image("file:///opt/images/redis")
    .memory_limit(256 * 1024 * 1024); // 256 MiB
```

#### `ContainerBuilder::cpu_shares`

```rust
pub const fn cpu_shares(mut self, shares: u64) -> Self
```

Sets the relative CPU weight via cgroup v2 `cpu.weight`. Higher values get proportionally more CPU time when the system is under contention. The default kernel value is 1024.

```rust
let builder = ContainerBuilder::new("batch")
    .image("file:///opt/images/batch")
    .cpu_shares(512); // half priority
```

#### `ContainerBuilder::readonly_rootfs`

```rust
pub const fn readonly_rootfs(mut self, readonly: bool) -> Self
```

Controls whether the container root filesystem is mounted read-only. Defaults to `true` for security — only explicitly declared volumes are writable. Set to `false` only when the application requires write access to the rootfs.

```rust
let builder = ContainerBuilder::new("dev")
    .image("file:///opt/images/devbox")
    .readonly_rootfs(false); // writable rootfs for development
```

#### `ContainerBuilder::build`

```rust
pub fn build(self) -> Result<Container>
```

Validates the builder configuration and produces a `Container` instance. The container is in the `Created` state — it is not started automatically.

**Errors:**

- `ContainustError::Config` — if the `image` field has not been set.

```rust
use containust_sdk::builder::ContainerBuilder;
use containust_common::error::ContainustError;

let result = ContainerBuilder::new("broken").build();

match result {
    Err(ContainustError::Config { message }) => {
        assert_eq!(message, "image source is required");
    }
    _ => panic!("expected Config error"),
}
```

### Complete Builder Pattern

```rust
use containust_sdk::builder::ContainerBuilder;

fn main() -> anyhow::Result<()> {
    let container = ContainerBuilder::new("production-api")
        .image("file:///opt/images/api-v2")
        .command(vec!["./api-server".into(), "--port".into(), "8080".into()])
        .env("RUST_LOG", "info")
        .env("DATABASE_URL", "postgres://db:5432/prod")
        .env("REDIS_URL", "redis://cache:6379")
        .memory_limit(512 * 1024 * 1024)  // 512 MiB
        .cpu_shares(2048)                  // double priority
        .readonly_rootfs(true)             // secure default
        .build()?;

    println!("Built container: {}", container.id);
    Ok(())
}
```

---

## GraphResolver

`containust_sdk::graph_resolver::GraphResolver` — Validates and resolves component dependency graphs from `.ctst` composition files.

### Method Reference

| Method             | Signature                                             | Description                                  |
|--------------------|-------------------------------------------------------|----------------------------------------------|
| `new`              | `fn new() -> Self`                                    | Create an empty graph resolver               |
| `load_ctst`        | `fn load_ctst(&mut self, path: &Path) -> Result<()>`  | Parse and load a `.ctst` file                |
| `deployment_order` | `fn deployment_order(&self) -> Result<Vec<String>>`   | Compute topological deployment order         |

`GraphResolver` also implements `Default`.

### `GraphResolver::new`

```rust
pub fn new() -> Self
```

Creates a new empty resolver backed by an internal `DependencyGraph` from `containust-compose`.

### `GraphResolver::load_ctst`

```rust
pub fn load_ctst(&mut self, path: &std::path::Path) -> Result<()>
```

Parses a `.ctst` composition file, validates its components and connections, and populates the internal dependency graph. The path must point to a valid `.ctst` file on disk.

**Errors:**

- `ContainustError::Io` — file does not exist or is unreadable.
- `ContainustError::Config` — syntax or validation error in the `.ctst` file.

### `GraphResolver::deployment_order`

```rust
pub fn deployment_order(&self) -> Result<Vec<String>>
```

Performs a topological sort on the dependency graph and returns component names in safe deployment order (dependencies before dependents).

**Errors:**

- Returns an error if the graph contains a cycle (circular dependency).

### Complete Example

```rust
use containust_sdk::graph_resolver::GraphResolver;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let mut resolver = GraphResolver::new();
    resolver.load_ctst(Path::new("infrastructure.ctst"))?;

    let order = resolver.deployment_order()?;
    println!("Deployment plan ({} components):", order.len());

    for (i, component) in order.iter().enumerate() {
        println!("  {}. {}", i + 1, component);
    }

    Ok(())
}
```

### Error Handling: Cycle Detection

```rust
use containust_sdk::graph_resolver::GraphResolver;
use std::path::Path;

fn deploy(path: &Path) -> anyhow::Result<()> {
    let mut resolver = GraphResolver::new();
    resolver.load_ctst(path)?;

    match resolver.deployment_order() {
        Ok(order) => {
            for name in &order {
                println!("Deploying: {name}");
            }
        }
        Err(e) => {
            eprintln!("Cannot resolve deployment order: {e}");
            eprintln!("Check for circular CONNECT directives in your .ctst file.");
        }
    }

    Ok(())
}
```

---

## EventListener

`containust_sdk::event::EventListener` — Subscribes to container lifecycle events for monitoring and automation.

### Creating a Listener

```rust
use containust_sdk::event::EventListener;

let listener = EventListener::new();
```

`EventListener` also implements `Default`:

```rust
let listener = EventListener::default();
```

### ContainerEvent Variants

| Variant          | Fields                                                         | Description                            |
|------------------|----------------------------------------------------------------|----------------------------------------|
| `StateChange`    | `container_id: ContainerId`, `from: ContainerState`, `to: ContainerState` | A container transitioned between states |
| `MetricsUpdate`  | `container_id: ContainerId`                                    | New metrics data is available           |

### Subscribing to Events (Future API)

The event subscription API follows an async callback pattern. When the `subscribe` method lands, usage will look like this:

```rust
use containust_sdk::event::{EventListener, ContainerEvent};
use containust_common::types::ContainerState;

async fn monitor_containers() {
    let listener = EventListener::new();

    // Future API — subscribe with a callback
    // listener.subscribe(|event: ContainerEvent| {
    //     match event {
    //         ContainerEvent::StateChange { container_id, from, to } => {
    //             println!("[{container_id}] {from} -> {to}");
    //             if to == ContainerState::Failed {
    //                 eprintln!("ALERT: container {container_id} has failed!");
    //             }
    //         }
    //         ContainerEvent::MetricsUpdate { container_id } => {
    //             println!("[{container_id}] metrics updated");
    //         }
    //     }
    // });
}
```

### Async Integration with Tokio

The `EventListener` is designed for use with the `tokio` async runtime. When the async API is finalized, integration will follow this pattern:

```rust
use containust_sdk::event::{EventListener, ContainerEvent};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = EventListener::new();

    // Future pattern: channel-based event consumption
    // let (tx, mut rx) = mpsc::unbounded_channel::<ContainerEvent>();
    //
    // tokio::spawn(async move {
    //     while let Some(event) = rx.recv().await {
    //         match event {
    //             ContainerEvent::StateChange { container_id, from, to } => {
    //                 tracing::info!(%container_id, %from, %to, "state change");
    //             }
    //             ContainerEvent::MetricsUpdate { container_id } => {
    //                 tracing::debug!(%container_id, "metrics update");
    //             }
    //         }
    //     }
    // });

    Ok(())
}
```

---

## Domain Types

All domain primitives live in `containust_common::types`.

### ContainerId

Unique identifier for a container instance. Wraps a `String` internally.

```rust
use containust_common::types::ContainerId;

// Create from a known name
let id = ContainerId::new("my-container");
assert_eq!(id.as_str(), "my-container");

// Generate a random UUID-based ID
let random_id = ContainerId::generate();
println!("Generated: {random_id}"); // Display trait prints the inner string
```

**Derives:** `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `Serialize`, `Deserialize`

| Method       | Signature                                    | Description                     |
|--------------|----------------------------------------------|---------------------------------|
| `new`        | `fn new(id: impl Into<String>) -> Self`      | Create from a string            |
| `generate`   | `fn generate() -> Self`                      | Generate a random UUID v4 ID    |
| `as_str`     | `fn as_str(&self) -> &str`                   | Borrow the inner string         |
| `Display`    | formats as the raw string value              | `"my-container"`                |

### ImageId

Unique identifier for a container image.

```rust
use containust_common::types::ImageId;

let image = ImageId::new("alpine-3.19");
println!("Image: {image}");         // "alpine-3.19"
println!("Raw: {}", image.as_str());
```

**Derives:** `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `Serialize`, `Deserialize`

| Method    | Signature                                    | Description              |
|-----------|----------------------------------------------|--------------------------|
| `new`     | `fn new(id: impl Into<String>) -> Self`      | Create from a string     |
| `as_str`  | `fn as_str(&self) -> &str`                   | Borrow the inner string  |
| `Display` | formats as the raw string value              | `"alpine-3.19"`          |

### Sha256Hash

SHA-256 digest used for content-addressable storage and integrity verification. Validates that the input is exactly 64 hexadecimal characters.

```rust
use containust_common::types::Sha256Hash;

let hash = Sha256Hash::from_hex(
    "a3f2b8c9d1e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0"
)?;

println!("Hex: {}", hash.as_hex());  // raw 64-char hex
println!("Full: {hash}");            // "sha256:a3f2b8c9d1..."
```

**Validation rules:**

- Must be exactly 64 characters long.
- Every character must be an ASCII hex digit (`0-9`, `a-f`, `A-F`).
- Invalid input returns `ContainustError::Config`.

```rust
use containust_common::types::Sha256Hash;
use containust_common::error::ContainustError;

// Too short — fails validation
let result = Sha256Hash::from_hex("abc123");
assert!(matches!(result, Err(ContainustError::Config { .. })));
```

**Derives:** `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `Serialize`, `Deserialize`

| Method     | Signature                                                  | Description                    |
|------------|------------------------------------------------------------|--------------------------------|
| `from_hex` | `fn from_hex(hex: impl Into<String>) -> Result<Self>`      | Validate and create from hex   |
| `as_hex`   | `fn as_hex(&self) -> &str`                                 | Borrow the raw hex string      |
| `Display`  | formats as `"sha256:<hex>"`                                | `"sha256:a3f2b8c9..."`         |

### ResourceLimits

Resource constraints applied to a container via cgroup v2.

```rust
use containust_common::types::ResourceLimits;

let limits = ResourceLimits {
    cpu_shares: Some(1024),
    memory_bytes: Some(256 * 1024 * 1024), // 256 MiB
    io_weight: Some(500),
};

// Default: all fields are None (no limits)
let unlimited = ResourceLimits::default();
assert_eq!(unlimited.cpu_shares, None);
```

**Derives:** `Debug`, `Clone`, `Default`, `PartialEq`, `Eq`, `Serialize`, `Deserialize`

| Field          | Type           | Default | Description                            |
|----------------|----------------|---------|----------------------------------------|
| `cpu_shares`   | `Option<u64>`  | `None`  | Relative CPU weight                    |
| `memory_bytes` | `Option<u64>`  | `None`  | Memory limit in bytes                  |
| `io_weight`    | `Option<u16>`  | `None`  | Block I/O weight (1–10000)             |

**Typical values:**

| Workload      | `cpu_shares` | `memory_bytes`           | `io_weight` |
|---------------|--------------|--------------------------|-------------|
| Low-priority  | 256          | 64 MiB (`67_108_864`)    | 100         |
| Standard      | 1024         | 256 MiB (`268_435_456`)  | 500         |
| High-priority | 2048         | 1 GiB (`1_073_741_824`)  | 1000        |
| Database      | 4096         | 4 GiB (`4_294_967_296`)  | 5000        |

### ContainerState

Lifecycle state of a container. Represents the finite state machine governing container execution.

**Derives:** `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `Serialize`, `Deserialize`

| Variant   | Display string | Description                              |
|-----------|----------------|------------------------------------------|
| `Created` | `"created"`    | Container configured but not yet started |
| `Running` | `"running"`    | Container process is actively executing  |
| `Stopped` | `"stopped"`    | Container was gracefully stopped         |
| `Failed`  | `"failed"`     | Container encountered a fatal error      |

**State machine transitions:**

```
              ┌──────────┐
              │ Created  │
              └────┬─────┘
                   │ start()
                   ▼
              ┌──────────┐
         ┌────│ Running  │────┐
         │    └──────────┘    │
         │ stop()        error│
         ▼                    ▼
    ┌──────────┐        ┌──────────┐
    │ Stopped  │        │  Failed  │
    └──────────┘        └──────────┘
```

Valid transitions:

- `Created` → `Running` (on start)
- `Running` → `Stopped` (on graceful stop)
- `Running` → `Failed` (on unrecoverable error)

Invalid transitions (these never occur):

- `Stopped` → `Running`
- `Failed` → `Running`
- `Created` → `Stopped`

---

## Configuration

`containust_common::config::ContainustConfig` — Global configuration for the Containust runtime.

### Fields and Defaults

| Field            | Type              | Default                               | Description                              |
|------------------|-------------------|---------------------------------------|------------------------------------------|
| `data_dir`       | `PathBuf`         | `/var/lib/containust`                 | Base directory for state and data        |
| `state_file`     | `PathBuf`         | `/var/lib/containust/state.json`      | Path to the state index file             |
| `offline`        | `bool`            | `false`                               | Block all outbound network access        |
| `default_limits` | `ResourceLimits`  | All `None`                            | Default resource limits for containers   |

### Creating a Default Config

```rust
use containust_common::config::ContainustConfig;

let config = ContainustConfig::default();
assert_eq!(config.data_dir.to_str().unwrap(), "/var/lib/containust");
assert!(!config.offline);
```

### Customizing Configuration

```rust
use containust_common::config::ContainustConfig;
use containust_common::types::ResourceLimits;
use std::path::PathBuf;

let config = ContainustConfig {
    data_dir: PathBuf::from("/opt/containust/data"),
    state_file: PathBuf::from("/opt/containust/data/state.json"),
    offline: true,
    default_limits: ResourceLimits {
        cpu_shares: Some(1024),
        memory_bytes: Some(512 * 1024 * 1024),
        io_weight: None,
    },
};
```

### Environment Variable Overrides

The CLI layer reads these environment variables and overrides the corresponding config fields:

| Variable                | Config Field    | Example                            |
|-------------------------|-----------------|-------------------------------------|
| `CONTAINUST_DATA_DIR`   | `data_dir`      | `CONTAINUST_DATA_DIR=/opt/data`     |
| `CONTAINUST_STATE_FILE` | `state_file`    | `CONTAINUST_STATE_FILE=/tmp/st.json`|
| `CONTAINUST_OFFLINE`    | `offline`       | `CONTAINUST_OFFLINE=true`           |

---

## Error Handling

All fallible SDK operations return `containust_common::error::Result<T>`, which is an alias for `std::result::Result<T, ContainustError>`.

### Error Variants

#### `ContainustError::Io`

**When it occurs:** File not found, permission denied on disk, read/write failure.

**Fields:** `path: PathBuf`, `source: std::io::Error`

**Display:** `"I/O error at /path/to/file: <io error>"`

```rust
ContainustError::Io { path, source } => {
    tracing::error!(%path, %source, "filesystem operation failed");
}
```

#### `ContainustError::Config`

**When it occurs:** Invalid configuration value, missing required builder field, malformed `.ctst` syntax.

**Fields:** `message: String`

**Display:** `"invalid configuration: <message>"`

```rust
ContainustError::Config { message } => {
    tracing::error!(%message, "configuration error — check your inputs");
}
```

#### `ContainustError::NotFound`

**When it occurs:** A container, image, or layer ID does not exist in storage.

**Fields:** `kind: &'static str`, `id: String`

**Display:** `"<kind> not found: <id>"`

```rust
ContainustError::NotFound { kind, id } => {
    tracing::warn!(resource_type = kind, %id, "resource not found");
}
```

#### `ContainustError::HashMismatch`

**When it occurs:** SHA-256 verification failed for a downloaded or loaded image/layer. Indicates corruption or tampering.

**Fields:** `resource: String`, `expected: String`, `actual: String`

**Display:** `"hash mismatch for <resource>: expected <expected>, got <actual>"`

```rust
ContainustError::HashMismatch { resource, expected, actual } => {
    tracing::error!(
        %resource, %expected, %actual,
        "integrity violation — possible tampering"
    );
}
```

#### `ContainustError::PermissionDenied`

**When it occurs:** Missing Linux capabilities, insufficient privileges for namespace/cgroup operations.

**Fields:** `message: String`

**Display:** `"permission denied: <message>"`

```rust
ContainustError::PermissionDenied { message } => {
    tracing::error!(%message, "insufficient privileges");
    eprintln!("Try running with appropriate capabilities or as root.");
}
```

#### `ContainustError::Serialization`

**When it occurs:** Failed to serialize/deserialize the state file or configuration JSON.

**Fields:** `source: serde_json::Error`

**Display:** `"serialization error: <source>"`

```rust
ContainustError::Serialization { source } => {
    tracing::error!(%source, "state file corruption — consider resetting state");
}
```

### Complete Error Handling Example

```rust
use containust_sdk::builder::ContainerBuilder;
use containust_common::error::ContainustError;

fn create_container(name: &str, image: &str) -> anyhow::Result<()> {
    let result = ContainerBuilder::new(name)
        .image(image)
        .memory_limit(128 * 1024 * 1024)
        .build();

    match result {
        Ok(container) => {
            println!("Container ready: {}", container.id);
            Ok(())
        }
        Err(ContainustError::Config { message }) => {
            eprintln!("Configuration error: {message}");
            eprintln!("Verify image URI and builder parameters.");
            Err(anyhow::anyhow!("invalid config: {message}"))
        }
        Err(ContainustError::NotFound { kind, id }) => {
            eprintln!("{kind} '{id}' does not exist.");
            eprintln!("Check that the image is available locally.");
            Err(anyhow::anyhow!("{kind} not found: {id}"))
        }
        Err(ContainustError::PermissionDenied { message }) => {
            eprintln!("Permission denied: {message}");
            eprintln!("Ensure the process has required Linux capabilities.");
            Err(anyhow::anyhow!("permission denied: {message}"))
        }
        Err(e) => {
            eprintln!("Unexpected error: {e}");
            Err(e.into())
        }
    }
}
```

---

## Patterns and Best Practices

### Builder Pattern Usage

Always chain builder calls fluently and call `.build()` last. The builder consumes `self` on each call (move semantics), ensuring compile-time linearity.

```rust
use containust_sdk::builder::ContainerBuilder;

fn build_service(name: &str, image: &str, port: u16) -> anyhow::Result<()> {
    let container = ContainerBuilder::new(name)
        .image(image)
        .command(vec!["./server".into()])
        .env("PORT", port.to_string())
        .memory_limit(256 * 1024 * 1024)
        .cpu_shares(1024)
        .build()?;

    println!("Service ready: {}", container.id);
    Ok(())
}
```

### Error Propagation with `?` and `anyhow`

In application code, use `anyhow::Result` and the `?` operator for concise error propagation. `ContainustError` implements `std::error::Error`, so it converts automatically.

```rust
use containust_sdk::builder::ContainerBuilder;
use containust_sdk::graph_resolver::GraphResolver;
use std::path::Path;

fn deploy_stack(ctst_path: &Path) -> anyhow::Result<()> {
    let mut resolver = GraphResolver::new();
    resolver.load_ctst(ctst_path)?;

    for name in resolver.deployment_order()? {
        let _container = ContainerBuilder::new(&name)
            .image(format!("file:///opt/images/{name}"))
            .build()?;
        println!("Deployed: {name}");
    }

    Ok(())
}
```

### Logging with `tracing` Integration

The SDK uses `tracing` internally. Configure a subscriber in your binary crate to capture structured logs.

```rust
use tracing_subscriber::EnvFilter;

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(true)
        .json()
        .init();
}

fn main() -> anyhow::Result<()> {
    init_logging();
    tracing::info!("Containust SDK initialized");
    Ok(())
}
```

Set the log level at runtime via the `RUST_LOG` environment variable:

```bash
RUST_LOG=containust_sdk=debug,containust_common=info cargo run
```

### Thread Safety Guarantees

All SDK public types are `Send + Sync` where appropriate:

- `ContainerBuilder` is `Send` (can be moved to another thread before `build()`).
- `GraphResolver` is `Send + Sync` (safe to share behind `Arc`).
- `EventListener` is `Send + Sync` (designed for async runtimes).
- All domain types (`ContainerId`, `ImageId`, `Sha256Hash`, `ResourceLimits`, `ContainerState`) are `Send + Sync`.

### Graceful Shutdown Pattern

Use `tokio::signal` to handle `SIGTERM`/`SIGINT` and clean up containers before exit.

```rust
use tokio::signal;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ... set up containers ...

    println!("Running. Press Ctrl+C to stop.");
    signal::ctrl_c().await?;

    println!("Shutting down...");
    // container.stop()?;
    // Clean up resources, flush state

    println!("Shutdown complete.");
    Ok(())
}
```

---

## Feature Flags

| Feature   | Crate Dependency   | Description                                                       | Default |
|-----------|--------------------|-------------------------------------------------------------------|---------|
| (default) | —                  | Core SDK: `ContainerBuilder`, `GraphResolver`, `EventListener`    | On      |
| `ebpf`    | `containust-ebpf`  | eBPF-powered monitoring: syscall tracing, file access, network    | Off     |

The `ebpf` feature requires:

- Linux kernel 5.15+
- `CAP_BPF` and `CAP_PERFMON` capabilities (or root)
- The `aya` crate (v0.13)

Enable it only on supported platforms:

```toml
[target.'cfg(target_os = "linux")'.dependencies]
containust-sdk = { git = "https://github.com/RemiPelloux/Containust.git", features = ["ebpf"] }
```

---

## Full Working Examples

### Example 1: Single Container Lifecycle

Build and inspect a single container from image to created state.

```rust
use containust_sdk::builder::ContainerBuilder;
use containust_common::types::ContainerState;

fn main() -> anyhow::Result<()> {
    let container = ContainerBuilder::new("alpine-shell")
        .image("file:///opt/images/alpine")
        .command(vec!["/bin/sh".into(), "-c".into(), "echo hello && sleep 30".into()])
        .env("TERM", "xterm-256color")
        .memory_limit(32 * 1024 * 1024)  // 32 MiB
        .cpu_shares(512)
        .readonly_rootfs(true)
        .build()?;

    println!("Container: {}", container.id);
    println!("State: {}", ContainerState::Created);
    println!("Memory: {:?} bytes", container.limits.memory_bytes);
    println!("CPU: {:?} shares", container.limits.cpu_shares);

    // Lifecycle: start -> wait -> stop
    // container.start()?;    // Created -> Running
    // container.wait()?;     // block until exit
    // container.stop()?;     // Running -> Stopped

    Ok(())
}
```

### Example 2: Multi-Container Composition via `.ctst`

Load a composition file, resolve the dependency graph, and deploy components in order.

```rust
use containust_sdk::graph_resolver::GraphResolver;
use containust_sdk::builder::ContainerBuilder;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let ctst_path = Path::new("stack.ctst");

    // Resolve dependencies
    let mut resolver = GraphResolver::new();
    resolver.load_ctst(ctst_path)?;

    let order = resolver.deployment_order()?;
    println!("Resolved {} components:", order.len());

    // Deploy each component in topological order
    for name in &order {
        let container = ContainerBuilder::new(name)
            .image(format!("file:///opt/images/{name}"))
            .memory_limit(256 * 1024 * 1024)
            .cpu_shares(1024)
            .build()?;

        println!("  Deploying: {} (id: {})", name, container.id);
        // container.start()?;
    }

    println!("Stack deployment complete.");
    Ok(())
}
```

### Example 3: Event Monitoring

Set up an event listener to monitor container state changes and metrics updates.

```rust
use containust_sdk::event::{EventListener, ContainerEvent};
use containust_common::types::ContainerState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = EventListener::new();

    // Future API pattern:
    // listener.subscribe(|event: ContainerEvent| {
    //     match event {
    //         ContainerEvent::StateChange { container_id, from, to } => {
    //             println!("[STATE] {container_id}: {from} -> {to}");
    //
    //             match to {
    //                 ContainerState::Failed => {
    //                     eprintln!("ALERT: {container_id} entered failed state!");
    //                 }
    //                 ContainerState::Stopped => {
    //                     println!("Container {container_id} stopped gracefully.");
    //                 }
    //                 _ => {}
    //             }
    //         }
    //         ContainerEvent::MetricsUpdate { container_id } => {
    //             println!("[METRICS] {container_id}: new data available");
    //         }
    //     }
    // });

    println!("Event listener created: {listener:?}");
    Ok(())
}
```

### Example 4: Error Recovery with Retry

Robust container creation with exponential backoff on transient failures.

```rust
use containust_sdk::builder::ContainerBuilder;
use containust_common::error::ContainustError;
use std::time::Duration;
use std::thread;

fn create_with_retry(
    name: &str,
    image: &str,
    max_retries: u32,
) -> anyhow::Result<()> {
    let mut attempt = 0;

    loop {
        let result = ContainerBuilder::new(name)
            .image(image)
            .memory_limit(128 * 1024 * 1024)
            .cpu_shares(1024)
            .readonly_rootfs(true)
            .build();

        match result {
            Ok(container) => {
                println!("Container created: {} (attempt {})", container.id, attempt + 1);
                return Ok(());
            }
            Err(ContainustError::Config { .. }) => {
                return Err(anyhow::anyhow!(
                    "configuration error is not retryable — fix inputs"
                ));
            }
            Err(ContainustError::PermissionDenied { .. }) => {
                return Err(anyhow::anyhow!(
                    "permission denied is not retryable — check capabilities"
                ));
            }
            Err(e) if attempt < max_retries => {
                let backoff = Duration::from_millis(100 * 2u64.pow(attempt));
                eprintln!(
                    "Attempt {} failed: {e}. Retrying in {:?}...",
                    attempt + 1,
                    backoff
                );
                thread::sleep(backoff);
                attempt += 1;
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "all {} retries exhausted. Last error: {e}",
                    max_retries + 1
                ));
            }
        }
    }
}

fn main() -> anyhow::Result<()> {
    create_with_retry("resilient-api", "file:///opt/images/api", 3)
}
```

---

*Built with Rust. Designed for sovereignty.*
