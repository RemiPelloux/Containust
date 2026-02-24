//! Example: Using the Containust SDK to programmatically create and manage containers.
//!
//! Run with:
//! ```bash
//! cargo run --example rust_sdk_example
//! ```

fn main() -> anyhow::Result<()> {
    // Initialize tracing for structured logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    // Build a container using the fluent API
    let container = containust_sdk::builder::ContainerBuilder::new("example-app")
        .image("file:///opt/images/alpine")
        .command(vec!["/bin/echo".into(), "Hello from Containust!".into()])
        .env("APP_ENV", "production")
        .memory_limit(64 * 1024 * 1024) // 64 MiB
        .cpu_shares(512)
        .readonly_rootfs(true)
        .build()?;

    println!("Container created: {}", container.id);
    println!("State: {}", container.state);
    println!("Command: {:?}", container.command);

    // In a real scenario, you would call container.start() here.
    // This requires Linux with proper namespace/cgroup support.

    Ok(())
}
