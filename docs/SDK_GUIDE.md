# SDK Guide

The Containust SDK (`containust-sdk`) provides a Rust library interface for programmatic container management.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
containust-sdk = { git = "https://github.com/containust/containust.git" }
```

## Core APIs

### ContainerBuilder

Fluent API for configuring and launching containers:

```rust
use containust_sdk::builder::ContainerBuilder;

fn main() -> anyhow::Result<()> {
    let container = ContainerBuilder::new("my-service")
        .image("file:///opt/images/alpine")
        .command(vec!["/bin/sh".into(), "-c".into(), "echo hello".into()])
        .env("MY_VAR", "my_value")
        .memory_limit(256 * 1024 * 1024)  // 256 MiB
        .cpu_shares(1024)
        .readonly_rootfs(true)
        .build()?;

    // Start the container
    // container.start()?;

    Ok(())
}
```

### GraphResolver

Load and resolve `.ctst` composition files programmatically:

```rust
use containust_sdk::graph_resolver::GraphResolver;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let mut resolver = GraphResolver::new();
    resolver.load_ctst(Path::new("infrastructure.ctst"))?;

    let order = resolver.deployment_order()?;
    for component in &order {
        println!("Deploy: {component}");
    }

    Ok(())
}
```

### EventListener

Subscribe to container lifecycle events:

```rust
use containust_sdk::event::{EventListener, ContainerEvent};

fn main() {
    let listener = EventListener::new();
    // listener.subscribe(|event| {
    //     match event {
    //         ContainerEvent::StateChange { container_id, from, to } => {
    //             println!("{container_id}: {from} -> {to}");
    //         }
    //         ContainerEvent::MetricsUpdate { container_id } => {
    //             println!("{container_id}: metrics updated");
    //         }
    //     }
    // });
}
```

## Error Handling

The SDK uses `containust_common::error::ContainustError` for typed errors. Wrap with `anyhow` in your application for ergonomic handling:

```rust
use containust_common::error::ContainustError;

fn handle_error(err: ContainustError) {
    match err {
        ContainustError::NotFound { kind, id } => {
            eprintln!("{kind} '{id}' not found");
        }
        ContainustError::PermissionDenied { message } => {
            eprintln!("Access denied: {message}");
        }
        other => eprintln!("Error: {other}"),
    }
}
```

## Thread Safety

All SDK types are `Send + Sync` where appropriate. The `EventListener` is designed for use with `tokio` async runtimes.

## Feature Flags

| Feature | Description | Default |
|---|---|---|
| `ebpf` | Enable eBPF-based monitoring in event streams | Off |
