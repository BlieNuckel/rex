use std::process::ExitCode;

use clap::Parser;

use crate::config::Config;

#[derive(Parser, Debug)]
pub(super) struct Options {}

impl Options {
    pub fn run(self, cfg: &mut Config) -> anyhow::Result<ExitCode> {
        cfg.token = None;
        cfg.server_token = None;
        cfg.save()?;
        println!("Logged out.");
        Ok(ExitCode::SUCCESS)
    }
}
