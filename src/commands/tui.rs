use std::process::ExitCode;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;

use crate::config::Config;
use crate::plex::auth;

#[derive(Parser, Debug, Default)]
pub(super) struct Options {}

impl Options {
    pub fn run(self, cfg: &mut Config) -> Result<ExitCode> {
        if cfg.token.is_none() && cfg.server_url.is_none() {
            cfg.token = Some(auth::login(cfg)?);
            cfg.save()?;
        }
        println!("Connecting…");
        let client = auth::connect(cfg)?;
        let section = cfg
            .music_section
            .clone()
            .context("no music section selected")?;
        crate::app::run(Arc::new(client), section)?;
        Ok(ExitCode::SUCCESS)
    }
}
