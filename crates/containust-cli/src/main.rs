//! # ctst — Containust CLI
//!
//! Daemon-less, sovereign container runtime.
//! Single binary for building, running, and managing containers.

#![allow(
    clippy::unnecessary_wraps,
    clippy::needless_pass_by_value,
    clippy::print_stdout,
    clippy::print_stderr
)]

mod commands;
mod converter;
mod output;

use clap::Parser;

use crate::commands::Cli;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    commands::execute(cli)
}
