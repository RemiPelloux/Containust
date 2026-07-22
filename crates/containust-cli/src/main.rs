//! # ctst — Containust CLI
//!
//! Daemon-less, sovereign container runtime.
//! Single binary for building, running, and managing containers.

#![allow(
    clippy::unnecessary_wraps,
    clippy::needless_pass_by_value,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::exit
)]

mod build_info;
mod commands;
mod converter;
mod output;

use clap::Parser;
use containust_common::codes;

use crate::commands::Cli;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    if let Err(error) = commands::execute(cli) {
        let class = codes::classify_message(&format!("{error:#}"));
        eprintln!("error[{}]: {error}", class.code);
        eprintln!("hint: {}", class.remediation);
        std::process::exit(class.exit_code);
    }
}
