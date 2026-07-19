//! `ctst logs` — View container logs.

use clap::Args;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Arguments for the `logs` command.
#[derive(Args, Debug)]
pub struct LogsArgs {
    /// Container ID or name.
    pub container: String,

    /// Follow log output.
    #[arg(short, long)]
    pub follow: bool,
}

/// Executes the `logs` command.
///
/// Retrieves and displays logs for the specified container.
///
/// # Errors
///
/// Returns an error if the container is not found or logs are unavailable.
pub fn execute(args: LogsArgs, options: &super::RuntimeOptions) -> anyhow::Result<()> {
    let engine = options.engine();
    let id = super::resolve_container_id(&engine, &args.container)?;
    if args.follow {
        return follow(&engine, &id);
    }
    let logs = engine.logs(&id).map_err(|e| anyhow::anyhow!("{e}"))?;

    if logs.is_empty() {
        println!("No logs available for container: {}", args.container);
    } else {
        print!("{logs}");
    }

    Ok(())
}

fn follow(
    engine: &containust_runtime::engine::Engine,
    id: &containust_common::types::ContainerId,
) -> anyhow::Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let signal = Arc::clone(&running);
    ctrlc::set_handler(move || signal.store(false, Ordering::Release))
        .map_err(|error| anyhow::anyhow!("failed to install Ctrl+C handler: {error}"))?;

    let mut offset = 0;
    while running.load(Ordering::Acquire) {
        let (content, next) =
            containust_runtime::logs::read_logs_from(engine.data_dir(), id.as_str(), offset)
                .map_err(|error| anyhow::anyhow!("{error}"))?;
        if !content.is_empty() {
            print!("{content}");
            std::io::stdout().flush()?;
        }
        offset = next;
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}
