use clap::Parser;
use commands::setup_logging;
use std::process::ExitCode;

mod commands;
mod config;
mod plex;

fn main() -> anyhow::Result<ExitCode> {
    let options = self::commands::Options::parse();

    setup_logging(options.log_level)?;
    let mut cfg = config::Config::load()?;

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting");
    options.run(&mut cfg)
}
