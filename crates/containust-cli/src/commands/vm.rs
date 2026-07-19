//! `ctst vm` — Virtual machine management commands.

use clap::Args;

/// Arguments for the `vm start` command.
#[derive(Args, Debug)]
pub struct VmStartArgs {
    /// Path to a custom kernel image.
    #[arg(long)]
    pub kernel: Option<String>,

    /// Path to a custom initramfs image.
    #[arg(long)]
    pub initramfs: Option<String>,
}

/// Arguments for the `vm stop` command.
#[derive(Args, Debug)]
pub struct VmStopArgs {
    /// Force kill the VM without graceful shutdown.
    #[arg(short, long)]
    pub force: bool,
}

/// Executes the `vm start` command.
///
/// Boots or ensures the QEMU-based VM backend is running.
///
/// # Errors
///
/// Returns an error if QEMU is not installed or the VM fails to start.
pub fn vm_start(args: VmStartArgs, options: &super::RuntimeOptions) -> anyhow::Result<()> {
    let engine = options.engine();

    let kernel = args.kernel.as_deref();
    let initramfs = args.initramfs.as_deref();
    engine
        .vm_start(kernel, initramfs)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    match (kernel, initramfs) {
        (Some(k), Some(i)) => {
            println!("VM started with custom kernel ({k}) and initramfs ({i}).");
        }
        _ => {
            println!("VM started with default assets.");
        }
    }

    Ok(())
}

/// Executes the `vm stop` command.
///
/// Stops the running QEMU-based VM backend.
///
/// # Errors
///
/// Returns an error if the VM cannot be stopped.
pub fn vm_stop(args: VmStopArgs, options: &super::RuntimeOptions) -> anyhow::Result<()> {
    let engine = options.engine();
    engine
        .vm_stop(args.force)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    if args.force {
        println!("VM force stopped.");
    } else {
        println!("VM stopped.");
    }
    Ok(())
}
